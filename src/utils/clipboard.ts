/**
 * Copy text to clipboard with fallback for older browsers
 */
export async function copyToClipboard(text: string): Promise<boolean> {
  try {
    // Modern clipboard API
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text);
      return true;
    }

    // Fallback for older browsers
    const textArea = document.createElement("textarea");
    textArea.value = text;
    textArea.style.position = "fixed";
    textArea.style.left = "-999999px";
    textArea.style.top = "-999999px";
    document.body.appendChild(textArea);
    textArea.focus();
    textArea.select();

    const successful = document.execCommand("copy");
    document.body.removeChild(textArea);

    return successful;
  } catch (error) {
    console.error("Failed to copy to clipboard:", error);
    return false;
  }
}

/**
 * Format error message for display
 */
export function formatErrorMessage(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }

  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

/**
 * Create a shareable error report
 */
export function createErrorReport(
  error: unknown,
  context?: Record<string, unknown>
): string {
  const timestamp = new Date().toISOString();
  const errorMessage = formatErrorMessage(error);

  let report = `CHAMP Error Report
Generated: ${timestamp}

Error Message:
${errorMessage}`;

  if (context) {
    report += `\n\nContext:
${JSON.stringify(context, null, 2)}`;
  }

  return report;
}
