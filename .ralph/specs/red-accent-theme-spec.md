# Red Accent Theme Specification

## Overview
Add a **red accent color theme** as a selectable option alongside the existing orange brand color. This allows users to personalize the look and feel of Vibe-Kanban by choosing red as the primary accent/brand color instead of the default orange. The feature applies to the **new design system** (`.new-design` scope) for both light and dark modes.

## Related BDD Scenarios
No existing BDD scenarios found for this feature.

## Figma Reference
No Figma design exists for this feature. The implementation should derive the red palette from the same HSL structure used by the current orange brand, substituting hue values.

### Proposed Red Palette

| Token | Current Orange (HSL) | Proposed Red (HSL) |
|-------|---------------------|-------------------|
| `--brand` | `25 82% 54%` | `0 72% 50%` |
| `--brand-hover` | `25 75% 62%` | `0 65% 58%` |
| `--brand-secondary` | `25 82% 37%` | `0 72% 35%` |
| `--text-on-brand` | `0 0% 100%` | `0 0% 100%` |

> These values should be tuned for accessibility (WCAG AA contrast ratios against both light and dark backgrounds).

---

## Specs

### Spec 1: Add AccentColor to Config Schema (Backend)

**Goal:** Extend the user config model to persist the selected accent color.

**Files to modify:**
- `crates/db/src/models/config.rs` (or wherever user config is stored)
- `crates/server/src/routes/config.rs` (if config routes need updating)
- New migration: `crates/db/migrations/*_add_accent_color_to_config.sql`

**Code Example:**
```rust
/// Accent color option for the UI theme
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum AccentColor {
    Orange, // default
    Red,
}
```

**Acceptance Criteria:**
- [ ] `AccentColor` enum is defined with `Orange` and `Red` variants
- [ ] `AccentColor` derives `TS` for TypeScript type generation
- [ ] User config model includes an `accent_color` field with default `Orange`
- [ ] Database migration adds `accent_color TEXT NOT NULL DEFAULT 'Orange'` column with CHECK constraint
- [ ] Config API returns `accent_color` in responses
- [ ] Config API accepts `accent_color` in update requests
- [ ] `pnpm run generate-types` produces the `AccentColor` enum in `shared/types.ts`
- [ ] `pnpm run prepare-db` succeeds with the new migration

---

### Spec 2: Define Red CSS Variables (Frontend Styles)

**Goal:** Add CSS variable definitions for the red accent color, activated by a data attribute or CSS class.

**Files to modify:**
- `frontend/src/styles/new/index.css`

**Code Example:**
```css
/* Red accent color overrides */
.new-design[data-accent="red"] {
  --brand: 0 72% 50%;
  --brand-hover: 0 65% 58%;
  --brand-secondary: 0 72% 35%;
  --text-on-brand: 0 0% 100%;
}

.new-design.dark[data-accent="red"],
.dark .new-design[data-accent="red"] {
  --brand: 0 72% 50%;
  --brand-hover: 0 65% 58%;
  --brand-secondary: 0 72% 35%;
  --text-on-brand: 0 0% 100%;
}
```

**Acceptance Criteria:**
- [ ] Red accent CSS variables are defined in `frontend/src/styles/new/index.css`
- [ ] Variables are scoped to `[data-accent="red"]` (or equivalent class) within `.new-design`
- [ ] Both light and dark mode have appropriate red accent variables
- [ ] Brand-related Tailwind utilities (`bg-brand`, `text-brand`, etc.) automatically use red when the attribute is set
- [ ] No changes to existing orange variables (orange remains the default)
- [ ] WCAG AA contrast ratio is maintained for `--text-on-brand` against `--brand` in both modes

---

### Spec 3: Apply Accent Color in Theme Provider / Design Scope

**Goal:** Read the user's accent color preference and apply the corresponding data attribute to the DOM.

**Files to modify:**
- `frontend/src/components/ui-new/scope/NewDesignScope.tsx`
- `frontend/src/components/ThemeProvider.tsx` (if accent needs to be in context)

**Code Example:**
```tsx
// NewDesignScope.tsx
export function NewDesignScope({ children }: NewDesignScopeProps) {
  const config = useConfig(); // or however user config is accessed
  const accent = config?.accent_color?.toLowerCase() ?? 'orange';

  return (
    <div
      className="new-design h-full"
      data-accent={accent !== 'orange' ? accent : undefined}
    >
      <PortalContainerContext.Provider value={ref}>
        {children}
      </PortalContainerContext.Provider>
    </div>
  );
}
```

**Acceptance Criteria:**
- [ ] `NewDesignScope` reads the user's `accent_color` from config
- [ ] `data-accent="red"` is applied to the `.new-design` wrapper when red is selected
- [ ] No `data-accent` attribute is set when orange is selected (default behavior)
- [ ] Accent color changes take effect immediately upon config update (no page reload)
- [ ] Portals/dialogs also inherit the accent color (via portal container)

---

### Spec 4: Add Accent Color Selector to Settings UI

**Goal:** Provide a UI control in General Settings for users to choose their accent color.

**Files to modify:**
- `frontend/src/components/ui-new/dialogs/settings/GeneralSettingsSection.tsx`
- `frontend/src/pages/settings/GeneralSettings.tsx` (legacy, if desired)

**Code Example:**
```tsx
// In GeneralSettingsSection.tsx, alongside the theme selector
<div>
  <Label>Accent Color</Label>
  <Select
    value={draft?.accent_color ?? 'Orange'}
    onValueChange={(value) => updateDraft({ accent_color: value })}
  >
    {Object.values(AccentColor).map((color) => (
      <SelectItem key={color} value={color}>
        <span className="flex items-center gap-2">
          <span
            className="w-3 h-3 rounded-full"
            style={{ backgroundColor: color === 'Red' ? '#d03030' : '#e87830' }}
          />
          {toPrettyCase(color)}
        </span>
      </SelectItem>
    ))}
  </Select>
</div>
```

**Acceptance Criteria:**
- [ ] Accent color selector is visible in the new design General Settings section
- [ ] Selector shows color swatches (small circles) next to each option name
- [ ] Selecting red and clicking Save persists the choice to the backend
- [ ] The accent color change is reflected immediately in the UI after save
- [ ] Default selection is "Orange" for new users / existing users without a preference
- [ ] The selector is placed near the existing Theme (Light/Dark/System) selector

---

## Implementation Order

1. **Spec 1** - Backend config schema (no dependencies, enables all other specs)
2. **Spec 2** - CSS variable definitions (no runtime dependency, can be done in parallel with Spec 1)
3. **Spec 3** - DOM application in NewDesignScope (depends on Spec 1 for config access)
4. **Spec 4** - Settings UI selector (depends on Specs 1 & 3)

## Overall Acceptance Criteria
- [ ] All unit tests pass (`cargo test --workspace`)
- [ ] Frontend type checks pass (`pnpm run check`)
- [ ] Backend compiles cleanly (`cargo check --workspace`)
- [ ] TypeScript types regenerated successfully (`pnpm run generate-types`)
- [ ] SQLx offline cache updated (`pnpm run prepare-db`)
- [ ] User can select "Red" accent in Settings and see the entire UI update
- [ ] User can switch back to "Orange" and see the default restored
- [ ] Theme switching (Light/Dark/System) works correctly with both accent colors
- [ ] Existing users without accent preference default to orange (no breaking change)
- [ ] No visual regressions in orange theme (default behavior unchanged)
