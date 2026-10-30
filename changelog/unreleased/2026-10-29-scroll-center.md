- **`Tx::scroll_center` and `Tx::scroll_center_in` reveal and vertically centre elements, virtual
  rows and `TextHighlight`s.** Passing `"row/mark"` mounts an unrealized virtual row `row` first
  and centres `mark` inside it, gliding on the scroll spring (`scroll_center_instant` and
  `scroll_center_in_instant` jump, as `Tx::instant` does). `Tx::scroll_key_to` also resolves
  `TextHighlight` keys.
