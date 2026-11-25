import OverviewCard from '@/components/providers/OverviewCard';
import InkBlock from '@/components/providers/InkBlock';
import {
  ModelBreakdownEntry,
  ProviderId,
  ProviderStatus,
  TrendData,
  UsageAlert,
  UsageSnapshot,
} from '@/types';

type ProviderOverviewProps = {
  snapshots: Record<ProviderId, UsageSnapshot | undefined>;
  trends: Record<ProviderId, TrendData | undefined>;
  modelBreakdowns: Record<ProviderId, ModelBreakdownEntry[]>;
  alerts: Record<ProviderId, UsageAlert[]>;
  statuses: Record<ProviderId, ProviderStatus | undefined>;
  onNavigate: (provider: ProviderId) => void;
};

const providers: ProviderId[] = ['claude', 'codex', 'antigravity', 'cursor'];

const ProviderOverview = ({
  snapshots,
  trends,
  modelBreakdowns,
  alerts,
  statuses,
  onNavigate,
}: ProviderOverviewProps) => (
  <>
  <InkBlock snapshots={snapshots} providers={providers} />
  <div className="qp-grid">
    {providers.map((id) => (
      <OverviewCard
        key={id}
        provider={id}
        snapshot={snapshots[id]}
        trend={trends[id]}
        breakdown={modelBreakdowns[id]}
        alerts={alerts[id]}
        status={statuses[id]}
        onClick={() => onNavigate(id)}
      />
    ))}
  </div>
  <p className="qp-footer">Actualizado en tiempo real · Directo a proveedores · Sin telemetría</p>
  </>
);

export default ProviderOverview;
