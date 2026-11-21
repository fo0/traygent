<script lang="ts">
  import { profileInitial } from './profiles';

  // A profile as its initial on a round tile (an account, unlike the model's
  // square `ModelBadge`). With `name` it is an image of that name, without
  // it decorative: its control names it. `warn` draws it in the warning
  // colour (a removed profile, #61).
  let {
    label,
    name,
    warn = false,
  }: { label: string; name?: string | undefined; warn?: boolean } = $props();
</script>

<span
  class="profile-mark"
  role={name ? 'img' : undefined}
  aria-label={name}
  aria-hidden={name ? undefined : 'true'}
  data-warn={warn ? 'true' : undefined}>{profileInitial(label)}</span
>
