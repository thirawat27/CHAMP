import { Component, ErrorInfo, ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error?: Error;
  errorInfo?: ErrorInfo;
}

/**
 * Error Boundary Component
 * 
 * Catches JavaScript errors anywhere in the child component tree,
 * logs those errors, and displays a fallback UI instead of crashing the whole app.
 * 
 * Usage:
 * ```tsx
 * <ErrorBoundary>
 *   <App />
 * </ErrorBoundary>
 * ```
 */
export class ErrorBoundary extends Component<Props, State> {
  public state: State = {
    hasError: false,
  };

  public static getDerivedStateFromError(error: Error): State {
    // Update state so the next render will show the fallback UI
    return { hasError: true, error };
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    // Log error details for debugging
    console.error("Uncaught error:", error);
    console.error("Error info:", errorInfo);
    
    this.setState({
      error,
      errorInfo,
    });
  }

  private handleReload = () => {
    window.location.reload();
  };

  public render() {
    if (this.state.hasError) {
      return (
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            minHeight: "100vh",
            padding: "2rem",
            textAlign: "center",
            backgroundColor: "var(--bg-primary, #f5f5f5)",
            color: "var(--text-primary, #333)",
          }}
        >
          <div
            style={{
              maxWidth: "600px",
              padding: "2rem",
              backgroundColor: "var(--bg-card, white)",
              borderRadius: "0.5rem",
              boxShadow: "0 4px 12px rgba(0, 0, 0, 0.1)",
            }}
          >
            <h1 style={{ fontSize: "1.5rem", marginBottom: "1rem", color: "var(--color-error, #ef4444)" }}>
              Something went wrong
            </h1>
            
            <p style={{ marginBottom: "1rem", fontSize: "0.875rem" }}>
              {this.state.error?.message || "An unexpected error occurred"}
            </p>

            {this.state.errorInfo && (
              <details style={{ marginBottom: "1.5rem", textAlign: "left" }}>
                <summary style={{ cursor: "pointer", marginBottom: "0.5rem", fontWeight: 600 }}>
                  Error Details
                </summary>
                <pre
                  style={{
                    fontSize: "0.75rem",
                    padding: "1rem",
                    backgroundColor: "var(--bg-card-secondary, #f9f9f9)",
                    borderRadius: "0.25rem",
                    overflow: "auto",
                    maxHeight: "200px",
                  }}
                >
                  {this.state.errorInfo.componentStack}
                </pre>
              </details>
            )}

            <button
              onClick={this.handleReload}
              style={{
                padding: "0.75rem 1.5rem",
                fontSize: "0.875rem",
                fontWeight: 600,
                color: "white",
                backgroundColor: "var(--color-primary, #3b82f6)",
                border: "none",
                borderRadius: "0.375rem",
                cursor: "pointer",
                transition: "background-color 0.2s",
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.backgroundColor = "var(--color-primary-hover, #2563eb)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor = "var(--color-primary, #3b82f6)";
              }}
            >
              Reload Application
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
