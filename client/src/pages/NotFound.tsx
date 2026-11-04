import { FileQuestion } from "lucide-react";
import { Link } from "react-router-dom";
import { EmptyState } from "../components/EmptyState";

export function NotFound() {
  return (
    <div className="page">
      <h1>Page not found</h1>
      <EmptyState
        icon={FileQuestion}
        title="This page does not exist"
        text="Check the address or go back to Home."
        action={
          <Link className="btn" to="/">
            Back to Home
          </Link>
        }
      />
    </div>
  );
}
