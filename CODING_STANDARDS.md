# Coding standards

The rules code in this repository follows. Code review checks every change against them.

## Styles (crates/forge-shared/src/theme.css)

- A global rule must not set a property that a component sets for itself. A press, hover or focus rule that writes `transform`,
  `box-shadow` or `border-color` on every button or field replaces the value the component chose, so the component jumps or
  loses its look while the rule applies. Use a property no component sets, or list the exception in the rule.
- A rule that sets `display` for a class must not name a class that an earlier rule hides by default (`display: none`). The later
  rule wins at equal specificity and shows the control everywhere, not only where the layout asked for it.
- When a box shows focus, the field inside it stays quiet. If both draw a border or a halo, the user sees two outlines.

## Maintaining this file

A rule enters this file when it appeared in a review finding the author fixed, in a fix commit, or in a solution record, and no
check covers it. State it as a condition that stays true without naming volatile numbers or symbols. Remove a rule when a check
starts to enforce it.
