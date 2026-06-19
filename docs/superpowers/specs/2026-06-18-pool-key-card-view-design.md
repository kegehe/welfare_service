# Pool Key Card View Design

## Goal

Change the pool key management area from table-only display to a default large-card view while keeping a table view for dense comparison.

## Design

- Keep the existing `PoolKeysTable` public props and events unchanged.
- Add a local `viewMode` state with `card` as the default and `table` as the alternate view.
- Place a segmented switch in the section header so users can move between card and table without leaving the current page.
- In card mode, show each upstream key as a wide management panel:
  - platform, ID, key prefix, status, circuit state
  - success rate, TPM/RPM limits, remaining TPM/RPM
  - OpenAI URL, Claude URL
  - models, source, note, created time
  - test, edit, toggle, delete actions
- Preserve the current table behavior for users who need compact scanning.
- Use a clear empty state with an add action when no pool keys exist.

## Validation

- Frontend build must pass.
- Existing backend tests and lint checks should still pass because no API behavior changes are expected.
