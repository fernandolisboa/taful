# Theme editor (spec §15.4, D7 amendment "Theme editor validation").

settings-theme-duplicate = Duplicate to edit
settings-theme-edit = Edit
theme-copy-name = { $name } (copy)

theme-editor = Theme editor
theme-editor-body = Changes show at once across the app, the table included. The checks are warnings only: you can save a theme that fails them.
theme-editor-name = Name: { $name }
theme-editor-rename = Rename
theme-editor-rename-hint = Type the new name. Enter saves, Esc cancels.
theme-editor-save = Save
theme-editor-discard = Discard changes
theme-editor-close = Close editor
theme-editor-delete = Delete theme
theme-editor-delete-confirm = Delete the theme “{ $name }”? This cannot be undone.
theme-editor-delete-yes = Delete
theme-editor-saved = Saved.
theme-editor-saved-warnings = Saved with { $count ->
    [one] one warning
   *[other] { $count } warnings
}.
theme-editor-unsaved = Unsaved changes.
theme-editor-preview = Preview
theme-editor-checks = Checks
theme-editor-ok = No warnings: this theme passes every rule the built-in themes follow.
theme-editor-warn-contrast = { $fg } on { $bg }: contrast { $ratio }:1, needs { $min }:1.
theme-editor-warn-red = { $token } reads as red. Taful avoids red everywhere.
theme-editor-warn-grades = Grades “{ $a }” and “{ $b }” are too close to tell apart.
theme-editor-hex-hint = Type a color as #rrggbb (or #rrggbbaa). Enter applies, Esc cancels.
theme-editor-hex-invalid = Not a color: use #rrggbb or #rrggbbaa.
theme-editor-hue = Hue
theme-editor-saturation = Saturation
theme-editor-lightness = Lightness
theme-editor-reset-token = Reset to saved

theme-group-surfaces = Surfaces and text
theme-group-table = Table
theme-group-cards = Cards and suits
theme-group-chips = Chips
theme-group-grades = Grades
theme-group-sizes = Spacing, corners and text sizes

theme-color-background = Background
theme-color-surface = Panels
theme-color-surface-raised = Raised panels and buttons
theme-color-border = Borders
theme-color-text = Text
theme-color-text-muted = Secondary text
theme-color-accent = Accent
theme-color-accent-muted = Accent, darker
theme-color-on-accent = Text on accent
theme-color-felt = Felt
theme-color-felt-line = Felt line
theme-color-rail = Rail
theme-color-rail-edge = Rail edge
theme-color-seat = Seat
theme-color-seat-active = Seat to act
theme-color-grid-dot = Layout grid dots
theme-color-card-face = Card face
theme-color-card-back = Card back
theme-color-suit-spades = Spades
theme-color-suit-hearts = Hearts
theme-color-suit-diamonds = Diamonds
theme-color-suit-clubs = Clubs
theme-color-chip-1 = Chip 1
theme-color-chip-5 = Chip 5
theme-color-chip-25 = Chip 25
theme-color-chip-100 = Chip 100
theme-color-chip-500 = Chip 500
theme-color-grade-best = Grade: best
theme-color-grade-correct = Grade: correct
theme-color-grade-inaccuracy = Grade: inaccuracy
theme-color-grade-mistake = Grade: mistake
theme-color-grade-blunder = Grade: blunder

theme-size-spacing-xs = Spacing XS
theme-size-spacing-sm = Spacing S
theme-size-spacing-md = Spacing M
theme-size-spacing-lg = Spacing L
theme-size-spacing-xl = Spacing XL
theme-size-spacing-xxl = Spacing XXL
theme-size-radii-sm = Corners S
theme-size-radii-md = Corners M
theme-size-radii-lg = Corners L
theme-size-radii-pill = Corners, chips and tags
theme-size-text-sm = Text S
theme-size-text-md = Text M
theme-size-text-lg = Text L
theme-size-text-xl = Text XL
