import { Inbox } from 'lucide-react';

type EmptyStateProps = {
  onOpenSettings?: () => void;
  title?: string;
  message?: string;
  ctaLabel?: string;
};

const EmptyState = ({
  onOpenSettings,
  title = 'No hay proveedores configurados',
  message = 'Activa al menos un proveedor en los ajustes para empezar a monitorear el uso.',
  ctaLabel = 'Abrir ajustes',
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
