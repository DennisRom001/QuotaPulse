import { AlertTriangle } from 'lucide-react';

type ErrorStateProps = {
  message: string;
  onRetry?: () => void;
};

const ErrorState = ({ message, onRetry }: ErrorStateProps) => (
  <div className="qp-state is-error" role="alert">
    <span className="qp-state-icon"><AlertTriangle size={16} /></span>
    <div className="qp-state-title">Something went wrong</div>
    <div className="qp-state-text">{message}</div>
    {onRetry && (
      <button className="qp-btn" type="button" onClick={onRetry}>
        Retry
      </button>
    )}
  </div>
);

export default ErrorState;
