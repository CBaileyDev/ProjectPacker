import { ErrorBoundary } from "react-error-boundary";
import Pack from "./routes/Pack";

function ErrorFallback({ error, resetErrorBoundary }: { error: Error; resetErrorBoundary: () => void }) {
  return (
    <div className="p-6 text-sm">
      <h2 className="font-semibold mb-2">Something went wrong</h2>
      <pre className="text-xs bg-red-50 dark:bg-red-950 p-2 rounded mb-3 overflow-auto">{error.message}</pre>
      <button type="button" onClick={resetErrorBoundary} className="px-3 py-1 border rounded">Reload</button>
    </div>
  );
}

export default function App() {
  return (
    <ErrorBoundary FallbackComponent={ErrorFallback}>
      <Pack />
    </ErrorBoundary>
  );
}
