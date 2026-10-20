<!-- LOVABLE:BEGIN -->
> [!IMPORTANT]
> This project is connected to [Lovable](https://lovable.dev). Avoid rewriting
> published git history — force pushing, or rebasing/amending/squashing commits
> that are already pushed — as it rewrites history on Lovable's side and the
> user will likely lose their project history.
>
> Commits you push to the connected branch sync back to Lovable and show up in
> the editor, so keep the branch in a working state.
<!-- LOVABLE:END -->

## Project rules
- The landing page lives in `src/components/kage/`; copy lives in `content.ts` so text edits never touch markup.
- three.js (pinned to 0.128 to match the source design's lighting) is dynamically imported inside `KageScene`'s effect so SSR never evaluates WebGL code.
- Component styles are plain classes in the `@layer components` block of `src/styles.css`, built only from the oklch tokens in `:root`.
