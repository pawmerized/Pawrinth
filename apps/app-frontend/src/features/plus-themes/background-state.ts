import { ref } from 'vue'

/** The chosen image couldn't be loaded, for example because the file was deleted. */
export const backgroundLoadFailed = ref(false)

/** Goes up when the image should be loaded again, even from the same path. */
export const backgroundLoadAttempt = ref(0)
