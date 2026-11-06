/** Skeleton list with the same geometry as the provider cards. */
const LoadingState = ({ rows = 4 }: { rows?: number }) => (
  <div className="qp-skeleton-list" aria-busy="true" aria-label="Cargando uso">
    {Array.from({ length: rows }, (_, i) => (
      <div key={i} className="qp-skeleton">
        <span className="qp-skel qp-skel-logo" />
        <span className="qp-skel" style={{ height: 12, width: '45%' }} />
        <span className="qp-skel" style={{ height: 10, width: '70%' }} />
        <span className="qp-skel" style={{ gridColumn: '1 / -1', height: 6, borderRadius: 9999 }} />
      </div>
    ))}
  </div>
);

export default LoadingState;
