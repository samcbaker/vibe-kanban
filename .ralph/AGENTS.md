# InfinitePay Dashboard Flutter - Agent Operations

## Commands
- **Bootstrap**: `melos bs`
- **Clean**: `melos clean && melos bs`
- **Test package**: `melos test:selective_unit_test`
- **Test changes**: `melos test:diff_without_coverage`
- **Analyze**: `melos analyze`

## Structure
- `app/lib/` - Main app (features/, pages/, delegates/, managers/)
- `microapps/` - Independent modules
- `packages/` - Shared code and isolated features (adapters/, apis/, features/, shared/)

## Architecture
- Clean Architecture: presentation → domain → infrastructure → data
- State: Cubit
- Communication: Contracts between microapps
- Errors: `Either<Error, Success>` pattern
