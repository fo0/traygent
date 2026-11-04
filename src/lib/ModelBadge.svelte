<script lang="ts">
  import { modelLetter } from './optionMarks';

  // The model as one letter on a small tile (`optionMarks → modelLetter`);
  // its full label is the accessible name.
  let { model, label }: { model: string; label: string } = $props();
</script>

<span class="model-badge" role="img" aria-label={`Modell: ${label}`}
  >{modelLetter(model)}</span
>
