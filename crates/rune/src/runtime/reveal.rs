//! `Tx::scroll_into_view` and `Tx::scroll_center`: revealing or centring a keyed element,
//! virtual row or `TextHighlight` inside its scroll viewport.

use super::*;

#[derive(Clone, Debug)]
pub(crate) struct ScrollReveal {
    pub scope: Option<String>,
    pub key: String,
    pub center: bool,
    pub animated: bool,
}

impl ScrollReveal {
    pub fn into_view(scope: Option<String>, key: String) -> Self {
        ScrollReveal { scope, key, center: false, animated: false }
    }

    pub fn center(scope: Option<String>, key: String, animated: bool) -> Self {
        ScrollReveal { scope, key, center: true, animated }
    }
}

pub(super) fn place(center: bool, offset: f64, top: f64, height: f64, viewport: f64, margin: f64) -> f64 {
    if center {
        (top - ((viewport - height) * 0.5).max(0.0)).max(0.0)
    } else {
        scroll_edge::reveal(offset, top, height, viewport, margin)
    }
}

impl<S: 'static> Runtime<S> {
    /// Scrolls so row or highlight `r.key` shows (or sits centred when `r.center`). A virtual row
    /// placed by estimates may measure differently once mounted, along with the rows around it,
    /// so this places it again from what the commit measured until it stays put.
    pub(super) fn reveal_row(&mut self, r: &ScrollReveal, instant: bool) {
        let scope = r.scope.as_deref();
        let glide = r.animated && !instant && !self.reduced_motion && !self.settled;
        let mut was: Option<(NodeId, f64, f64)> = None;
        let (window, first_was) = self.reveal_row_once(scope, &r.key, r.center, true, instant || glide);
        if first_was.is_some() {
            was = first_was;
        }
        if window {
            for _ in 0..8 {
                let (again, next_was) = self.reveal_row_once(scope, &r.key, r.center, false, instant || glide);
                if was.is_none() {
                    was = next_was;
                }
                if !again {
                    break;
                }
            }
        }
        if glide && let Some((id, was_offset, viewport)) = was {
            let end = self.scrolls.get(&id).map(|s| s.offset).unwrap_or(was_offset);
            if (end - was_offset).abs() > 0.5 {
                let reach = scroll_bottom::GLIDE_REACH * viewport;
                let start = was_offset.clamp(end - reach, end + reach);
                if let Some(st) = self.scrolls.get_mut(&id) {
                    st.offset = start;
                    st.v = 0.0;
                    st.glide_to = Some(end);
                    st.glide = Some(0.0);
                    st.shrink_glide = false;
                    st.pinned = false;
                    st.direct = true;
                }
                if (start - end).abs() > 0.5 && self.virtual_window_changed() {
                    self.commit(&Tx { instant: true, ..Tx::default() });
                }
            }
        }
        self.reveal_row_x(scope, &r.key);
    }

    /// One placement of [`Self::reveal_row`]: `(committed_window_change, Some((viewport_id, was_offset, viewport_h)))`.
    fn reveal_row_once(
        &mut self,
        scope: Option<&str>,
        key: &str,
        center: bool,
        first: bool,
        instant: bool,
    ) -> (bool, Option<(NodeId, f64, f64)>) {
        let target_node = self.nodes.iter().enumerate().find_map(|(i, row)| {
            if !row.named(key) || self.subtree_exiting(i) || !row.parent.is_none_or(|p| in_scope(&self.nodes[p].path, scope)) {
                return None;
            }
            Some((i, row.target[1], row.local.h, true))
        }).or_else(|| {
            let (i, [_, y0, _, y1]) = self.highlight_target_in(&self.nodes, scope, key)?;
            (!self.subtree_exiting(i)).then_some((i, y0, (y1 - y0).max(0.0), false))
        });
        let geometry = target_node.and_then(|(i, target_top, target_h, is_whole_node)| {
            let s = self.viewport_of(i)?;
            let parent = &self.nodes[s];
            let (laid_top, laid_h) = (target_top - parent.target[1], target_h);
            let (top, height) = std::iter::successors(Some(i), |&p| self.nodes[p].parent)
                .take_while(|&p| p != s)
                .find_map(|r| {
                    let c = self.nodes[r].parent?;
                    self.nodes[c].growth.as_ref()?;
                    let stack = self.stacks.get(&self.nodes[c].id)?;
                    let rid = self.nodes[r].id;
                    let mut count = 0usize;
                    let before_r: f32 = stack.order.iter()
                        .take_while(|&&id| id != rid)
                        .filter_map(|id| self.grows.get(id))
                        .map(|g| { count += 1; g.h })
                        .sum::<f32>() + stack.gap * count as f32;
                    let r_top = (self.nodes[c].target[1] - parent.target[1]) + stack.pad_top + before_r;
                    let r_h = self.grows.get(&rid).map_or(self.nodes[r].local.h, |g| g.h);
                    if i == r && is_whole_node {
                        Some((r_top, r_h))
                    } else {
                        let in_row = target_top - self.nodes[r].target[1];
                        Some((r_top + in_row, target_h))
                    }
                })
                .unwrap_or((laid_top, laid_h));
            let laid_end = self.scroll_children(s).map(|c| self.nodes[c].target[3] - parent.target[1]).reduce(f32::max).unwrap_or(0.0);
            Some((parent.path.clone(), f64::from(top), f64::from(height), f64::from(parent.local.h), f64::from(laid_top), f64::from(laid_h), f64::from(laid_end)))
        }).or_else(|| {
            self.nodes.iter().filter(|n| in_scope(&n.path, scope)).filter_map(|n| {
                let v = self.virtuals.get(&n.path)?;
                let index = v.index_of(key).or_else(|| {
                    key.rmatch_indices('/').find_map(|(at, _)| {
                        let (scope, row_key) = (&key[..at], &key[at + 1..]);
                        let mut keyed = n.path.rsplit('/').filter(|seg| !seg.starts_with('#'));
                        scope.rsplit('/').all(|seg| keyed.next() == Some(seg)).then(|| v.index_of(row_key))?
                    })
                }).or_else(|| {
                    // `"row_key/child_key"` names a child or highlight inside an unmounted row.
                    key.match_indices('/').find_map(|(at, _)| v.index_of(&key[..at]))
                })?;
                let row_key = &v.keys[index];
                let top = f64::from(v.top + v.pad_top + v.prefix(index));
                let height = f64::from(v.height(row_key));
                let end = f64::from(v.top + v.pad_top + v.prefix(v.keys.len()) + v.pad_bottom);
                Some((v.scroll_path.clone(), top, height, f64::from(v.viewport), top, height, end))
            }).next()
        });
        let Some((path, top, height, viewport, laid_top, laid_h, laid_end)) = geometry else { return (false, None) };
        // Stuck headers cover the viewport's top: land the row below them.
        let Some(cover) = self.stuck_cover(&path, key) else { return (false, None) };
        let (top, height) = (top - cover, height + cover);
        let (laid_top, laid_h) = (laid_top - cover, laid_h + cover);
        let margin = self.node(&path).map_or(0.0, |n| f64::from(n.scroll_edges.margin));
        let Some(&id) = self.path_ids.get(&path) else { return (false, None) };
        let Some(&s_idx) = self.node_index.get(&id) else { return (false, None) };
        let stick = self.nodes[s_idx].scroll.is_some_and(|s| s.stick_bottom);
        let max = self.scroll_overflow(s_idx).max(0.0);
        let Some(scroll) = self.scrolls.get_mut(&id) else { return (false, None) };
        let was_offset = scroll.offset;
        scroll.max = max;
        if first {
            scroll.v = 0.0;
            scroll.glide = None;
            scroll.glide_to = None;
        }
        let offset = place(center, scroll.offset, top, height, viewport, margin).clamp(0.0, scroll.max);
        let laid_max = (laid_end - viewport).max(0.0);
        let laid_to = place(center, scroll.offset, laid_top, laid_h, viewport, margin).clamp(0.0, laid_max);
        let at_end = stick && (scroll.pinned || offset >= scroll.max - 0.5) && laid_to >= laid_max - 0.5;
        if first {
            self.scroll_reveals.retain(|r| r.viewport != path);
            self.scroll_reveals.push(sticky::Reveal {
                scope: scope.map(str::to_owned),
                key: key.to_owned(),
                viewport: path.clone(),
                center,
                at_end,
            });
        } else if let Some(r) = self.scroll_reveals.iter_mut().find(|r| r.viewport == path) {
            r.at_end |= at_end;
        }
        let captured = first.then_some((id, was_offset, viewport));
        if (offset - scroll.offset).abs() < 0.5 {
            if first { scroll.pinned = false; }
            return (false, captured);
        }
        scroll.offset = offset;
        scroll.pinned = false;
        scroll.v = 0.0;
        scroll.glide = None;
        scroll.glide_to = None;
        scroll.direct = true;
        let window = self.virtual_window_changed();
        if window {
            self.commit(&Tx { instant, ..Tx::default() });
            if let Some(i) = self.nodes.iter().position(|n| n.id == id) {
                let max = self.scroll_overflow(i).max(0.0);
                if let Some(scroll) = self.scrolls.get_mut(&id) {
                    scroll.max = max;
                    scroll.pinned = false;
                }
            }
        }
        (window, captured)
    }
}
