# Red Theme Specification

## Overview
Add a new "Red" theme option to vibe-kanban alongside the existing "Light", "Dark", and "System" themes. The red theme will provide a distinct color scheme with red/dark red tones for users who prefer this aesthetic.

## Related BDD Scenarios
No existing BDD scenarios found for this feature.

## Figma Reference
No Figma design provided. Implementation follows existing theme patterns in the codebase.

## Specs

### Spec 1: Add RED to ThemeMode Enum (Rust)

**Goal:** Extend the ThemeMode enum to include a RED variant

**Files to modify:**
- `crates/server/src/bin/generate_types.rs` (or wherever ThemeMode is defined in Rust)

**Code Example:**
```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
pub enum ThemeMode {
    LIGHT,
    DARK,
    SYSTEM,
    RED,  // New variant
}
```

**Acceptance Criteria:**
- [ ] RED variant added to ThemeMode enum
- [ ] TypeScript types regenerated via `pnpm run generate-types`
- [ ] `shared/types.ts` contains updated ThemeMode with RED option

---

### Spec 2: Update ThemeProvider to Handle Red Theme

**Goal:** Modify ThemeProvider to apply the "red" CSS class when RED theme is selected

**Files to modify:**
- `frontend/src/components/ThemeProvider.tsx`

**Code Example:**
```tsx
useEffect(() => {
  const root = window.document.documentElement;

  root.classList.remove('light', 'dark', 'red');

  if (theme === ThemeMode.SYSTEM) {
    const systemTheme = window.matchMedia('(prefers-color-scheme: dark)')
      .matches
      ? 'dark'
      : 'light';
    root.classList.add(systemTheme);
    return;
  }

  root.classList.add(theme.toLowerCase());
}, [theme]);
```

**Acceptance Criteria:**
- [ ] ThemeProvider removes 'red' class along with 'light' and 'dark'
- [ ] ThemeProvider adds 'red' class when ThemeMode.RED is selected
- [ ] Theme switching between all four options works correctly

---

### Spec 3: Update theme.ts Utility

**Goal:** Update getActualTheme to handle RED theme mode

**Files to modify:**
- `frontend/src/utils/theme.ts`

**Code Example:**
```typescript
export function getActualTheme(
  themeMode: ThemeMode | undefined
): 'light' | 'dark' | 'red' {
  if (!themeMode || themeMode === ThemeMode.LIGHT) {
    return 'light';
  }

  if (themeMode === ThemeMode.SYSTEM) {
    return window.matchMedia('(prefers-color-scheme: dark)').matches
      ? 'dark'
      : 'light';
  }

  if (themeMode === ThemeMode.RED) {
    return 'red';
  }

  // ThemeMode.DARK
  return 'dark';
}
```

**Acceptance Criteria:**
- [ ] Return type updated to include 'red'
- [ ] RED theme mode returns 'red'
- [ ] Existing light/dark/system behavior unchanged

---

### Spec 4: Add Red Theme CSS Variables

**Goal:** Define CSS variables for the red theme in the stylesheet

**Files to modify:**
- `frontend/src/styles/new/index.css`

**Code Example:**
```css
/* Red mode - handles both .new-design.red and .red .new-design */
.new-design.red,
.red .new-design {
  color-scheme: dark;

  /* Internal variables (red) */
  --_background: 0 10% 10%;
  --_foreground: 0 0% 96%;
  --_primary: 0 70% 50%;
  --_primary-foreground: 0 0% 100%;
  --_secondary: 0 15% 12%;
  --_secondary-foreground: 0 0% 77%;
  --_muted: 0 15% 16%;
  --_muted-foreground: 0 0% 64%;
  --_accent: 0 50% 25%;
  --_accent-foreground: 0 0% 96%;
  --_destructive: 0 59% 57%;
  --_destructive-foreground: 0 0% 96%;
  --_border: 0 20% 20%;
  --_input: 0 15% 18%;
  --_ring: 0 70% 50%;

  /* Status (red theme) */
  --_success: 117 38% 50%;
  --_success-foreground: 117 38% 90%;
  --_warning: 32.2 95% 44.1%;
  --_warning-foreground: 32.2 95% 90%;
  --_info: 217.2 91.2% 59.8%;
  --_info-foreground: 217.2 91.2% 90%;
  --_neutral: 0 15% 20%;
  --_neutral-foreground: 0 0% 90%;

  /* Text (red theme) */
  --text-high: 0 0% 96%;
  --text-normal: 0 0% 77%;
  --text-low: 0 0% 56%;

  /* Background (red theme) */
  --bg-primary: 0 10% 10%;
  --bg-secondary: 0 15% 8%;
  --bg-panel: 0 15% 14%;

  /* Accent (red theme) - red brand color */
  --brand: 0 70% 50%;
  --brand-hover: 0 65% 58%;
  --brand-secondary: 0 70% 35%;
  --error: 30 80% 50%;
  --success: 117 38% 50%;

  /* Text on accent */
  --text-on-brand: 0 0% 100%;

  /* Console (red theme) */
  --_console-background: 0 10% 5%;
  --_console-foreground: 210 40% 98%;
  --_console-success: 138.5 76.5% 47.7%;
  --_console-error: 30 84.2% 60.2%;

  /* Syntax highlighting (red theme) */
  --_syntax-keyword: #ff7b72;
  --_syntax-function: #d2a8ff;
  --_syntax-constant: #79c0ff;
  --_syntax-string: #ffa5a5;
  --_syntax-variable: #ffa657;
  --_syntax-comment: #8b949e;
  --_syntax-tag: #7ee787;
  --_syntax-punctuation: #c9d1d9;
  --_syntax-deleted: #ffdcd7;
}
```

**Acceptance Criteria:**
- [ ] Red theme CSS variables defined for `.new-design.red` and `.red .new-design`
- [ ] Background colors have red/dark-red hues
- [ ] Brand accent color is red instead of orange
- [ ] All required CSS variable categories covered (text, background, accent, status, console, syntax)
- [ ] Theme is visually distinct and usable (sufficient contrast)

---

### Spec 5: Add i18n Translations (Optional)

**Goal:** Ensure "Red" theme label is translatable

**Files to modify:**
- `frontend/src/i18n/locales/en/settings.json`
- Other locale files as needed

**Code Example:**
The theme dropdown uses `toPrettyCase(theme)` which will convert "RED" to "Red" automatically. No i18n changes required unless custom labels are desired.

**Acceptance Criteria:**
- [ ] Theme appears as "Red" in the settings dropdown
- [ ] Label is readable and consistent with other theme options

## Implementation Order

1. **Spec 1** - Add RED to ThemeMode enum (Rust) - no dependencies, foundational change
2. **Spec 3** - Update theme.ts utility - depends on Spec 1 (type generation)
3. **Spec 2** - Update ThemeProvider - depends on Spec 1 (type generation)
4. **Spec 4** - Add Red Theme CSS Variables - depends on Spec 2 (CSS class applied)
5. **Spec 5** - Add i18n translations (optional) - can be done anytime

## Overall Acceptance Criteria

- [ ] RED option appears in Settings > Appearance > Theme dropdown
- [ ] Selecting "Red" applies the red theme immediately
- [ ] Red theme has dark background with red accent colors
- [ ] All UI elements are visible and usable in red theme
- [ ] Theme persists after page reload
- [ ] Switching between Light, Dark, System, and Red works correctly
- [ ] No TypeScript or build errors
- [ ] Existing tests pass (run `cargo test --workspace` and `pnpm run check`)
