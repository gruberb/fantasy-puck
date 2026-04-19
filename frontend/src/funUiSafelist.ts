/**
 * Tailwind v4 scans files under `src/` for class names but skips
 * `node_modules/`, so the arbitrary-value classes baked into the
 * `@gruberb/fun-ui` bundle
 * (`border-[var(--color-brutal-black)]` etc.) never land in the
 * output CSS. Without them, fun-ui components render unstyled — the
 * `LoadingSpinner` border ring turns invisible, Modal borders
 * disappear, the full brutalist palette collapses.
 *
 * Listing the class strings in a scanned source file is the
 * lowest-ceremony fix: Tailwind sees them here, emits the utilities,
 * fun-ui picks them up at runtime. Regenerate with:
 *
 *   grep -oE '[a-z][a-z-]*-\\[var\\(--color-brutal-[a-z]+\\)\\]' \\
 *     node_modules/@gruberb/fun-ui/dist/fun-ui.js | sort -u
 *
 * Not imported anywhere — existence as a source file is enough.
 */
export const FUN_UI_TAILWIND_SAFELIST = `
  bg-[var(--color-brutal-black)]
  bg-[var(--color-brutal-blue)]
  bg-[var(--color-brutal-cream)]
  bg-[var(--color-brutal-red)]
  bg-[var(--color-brutal-teal)]
  bg-[var(--color-brutal-yellow)]
  border-[var(--color-brutal-black)]
  border-[var(--color-brutal-cream)]
  border-[var(--color-brutal-red)]
  border-l-[var(--color-brutal-blue)]
  border-l-[var(--color-brutal-green)]
  border-l-[var(--color-brutal-orange)]
  border-l-[var(--color-brutal-red)]
  border-t-[var(--color-brutal-yellow)]
  text-[var(--color-brutal-black)]
  text-[var(--color-brutal-blue)]
  text-[var(--color-brutal-cream)]
  text-[var(--color-brutal-gray)]
  text-[var(--color-brutal-green)]
  text-[var(--color-brutal-orange)]
  text-[var(--color-brutal-red)]
  text-[var(--color-brutal-yellow)]
`;
