# Property exploration: observed slot map (tier 1)

Input for future `ordinary-form-patching` deltas. Every entry below was produced by a real
experiment gated through the 1C platform (8.3.27.2342, batch Designer dump/load) — see
`tests/explore/<id>/manifest.json` and `docs/../openspec` change `form-property-exploration-corpus`.

Observation channels per experiment:

- `observed.json` / `net-diff.json` — raw property-tree diff (mutated vs platform-normalized), net of the baseline normalization.
- `decoded-diff.json` — semantic diff through the oneform decoder: fires when the slot is an already-known property.
- `manifest.json` — outcome (`accept` / `reject` / `silent-drop`) and control survival.

## Confirmed mappings (known properties detected by the semantic channel)

| Slot | Control | Property | Evidence |
|---|---|---|---|
| `info[2][0][7]` | InputField | `ChoiceButton` (`ПолеВвода.КнопкаВыбора`) | `inputfield-flag-05`: decoded-diff `props.choiceButton` |
| `info[2][0][8]` | InputField | `ClearButton` (`КнопкаОчистки`) | `inputfield-flag-06`: decoded-diff `props.clearButton` |
| `geom[5]` | all controls | `Visible` | `smoke-visible-off` (already mapped; used as the conveyor's control experiment) |

## Platform-forced slots (normalization reverts the mutation)

These slots are read-only/computed for the current control state; the platform flips them back
during dump/load. They are NOT writable properties under the tested conditions.

| Slot | Control | Attempted | Result |
|---|---|---|---|
| `info[1][5]` | Button | 0 → 1 | reverted to 0 |
| `info[1][0][8]` | CheckBox | 1 → 0 | reverted to 1 |

## Structural markers (not properties)

| Slot | Controls | Evidence |
|---|---|---|
| `info[0]` | Button, CheckBox | 38-row rewrite of the whole control record |

## Open hypotheses (stored, accepted, no semantic signal yet)

26 further flag slots across InputField / CheckBox / Button / Table were toggled, accepted and
survived with no decoder-visible effect and no normalization revert. `Enabled` (`Доступность`) and
`ReadOnly` (`ТолькоПросмотр`) are not in the decoded tier-1 set; locating them needs the runtime
introspection channel (runner EPF reading `ЭлементыФормы.*` in thick client) or a targeted sweep of
the appearance blocks, both deferred to phase 2.
