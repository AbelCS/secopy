// What stopping a copy leaves behind, for the Cancel and quit questions. Only mentions the
// checksum file when the job writes one (RFD §5.5).

export function stopMessage(checksumFile: boolean): string {
  return checksumFile
    ? "Files already copied stay and are listed in the checksum file; the file in progress is removed."
    : "Files already copied stay; the file in progress is removed.";
}
