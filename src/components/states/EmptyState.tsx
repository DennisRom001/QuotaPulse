import { Inbox } from 'lucide-react';

type EmptyStateProps = {
  onOpenSettings?: () => void;
  title?: string;
  message?: string;
  ctaLabel?: string;
};

const EmptyState = ({
  onOpenSettings,
  title = 'No providers configured',
  message = 'Enable at least one provider in settings to start tracking usage.',
  ctaLabel = 'Open Settings',
}: EmptyStateProps) => (
  <div className="qp-state">
    <span className="qp-state-icon"><Inbox size={16} /></span>
    <div className="qp-state-title">{title}</div>
    <div className="qp-state-text">{message}</div>
    {onOpenSettings && (
      <button className="qp-btn" onClick={onOpenSettings} type="button">
        {ctaLabel}
      </button>
    )}
  </div>
);

export default EmptyState;
