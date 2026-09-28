/**
 * Safely extracts a string message from any caught error.
 *
 * @param error Caught error object (typically of type `unknown`)
 * @returns The message string of the error, or a stringified representation
 */
export function getErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}
