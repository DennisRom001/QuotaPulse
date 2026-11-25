import { Alert } from '@/components/icons';

type ErrorStateProps = {
  message: string;
  onRetry?: () => void;
};

const ErrorState = ({ message, onRetry }: ErrorStateProps) => (
  <div className="qp-state is-error" role="alert">
    <span className="qp-state-icon"><Alert size={16} /></span>
    <div className="qp-state-title">Algo salió mal</div>
    <div className="qp-state-text">{message}</div>
    {onRetry && (
      <button className="qp-btn" type="button" onClick={onRetry}>
        Reintentar
      </button>
    )}
  </div>
);

export default ErrorState;
