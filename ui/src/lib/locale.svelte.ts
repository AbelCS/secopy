// The language change signal (#181): every text `t()` draws reads it, so a new language
// redraws what's on screen in place, with nothing started again.
export const languageSignal = $state({ version: 0 });
