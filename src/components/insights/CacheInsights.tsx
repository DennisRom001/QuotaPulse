import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { CacheInsights as CacheInsightsData, ProviderId } from '@/types';
import { formatUsd } from '@/utils/format';
import { isTauriRuntime } from '@/utils/runtime';

type CacheInsightsProps = {
  provider: ProviderId;
  // Changes whenever fresh usage lands, so the panel refetches with it.
  refreshKey: unknown;
};

const pct = (ratio: number) => `${Math.round(ratio * 100)}%`;

// Saved per month at hit rate `h`, for one day's prompt volume, taking every
// miss as a cache write — the same assumption as the break-even line.
const monthlySaving = (tokensPerDay: number, h: number, [input, read, write]: [number, number, number]) =>
  ((tokensPerDay * 30) / 1_000_000) * (input - h * read - (1 - h) * write);

const roundSig = (usd: number) => {
  if (usd === 0) return 0;
  const step = 10 ** (Math.floor(Math.log10(Math.abs(usd))) - 1);
  return Math.round(usd / step) * step;
};

// Whole dollars from $10 up: the projection is rounded, so cents would lie.
const savingLabel = (usd: number) => {
  const abs = Math.abs(usd);
  const text = abs >= 10 && abs < 1_000 ? `$${Math.round(abs)}` : formatUsd(abs);
  return usd < 0 ? `−${text}` : text;
};

const CacheInsights = ({ provider, refreshKey }: CacheInsightsProps) => {
  const [data, setData] = useState<CacheInsightsData | null>(null);
  const [view, setView] = useState<'cached' | 'uncached'>('cached');
  const [assumed, setAssumed] = useState<number | null>(null);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    let cancelled = false;
    invoke<CacheInsightsData>('get_cache_insights', { provider })
      .then((next) => !cancelled && setData(next))
      .catch(() => !cancelled && setData(null));
    return () => {
      cancelled = true;
    };
  }, [provider, refreshKey]);

  // The slider starts at the real 7-day hit rate, per provider.
  useEffect(() => setAssumed(null), [provider]);

  const hit = assumed ?? data?.hit_ratio_7d ?? 0.8;

  // At the last 7 days' average daily volume (quiet days included), rounded to
  // two significant figures: a pace, not a promise.
  const projection = useMemo(() => {
    if (!data?.rates_per_m) return null;
    const days = data.daily_prompt_tokens_7d;
    const perDay = days.reduce((sum, t) => sum + t, 0) / Math.max(days.length, 1);
    if (perDay <= 0) return null;
    return roundSig(monthlySaving(perDay, hit, data.rates_per_m));
  }, [data, hit]);

  if (!data || (data.month_cost_usd <= 0 && data.hit_ratio_7d == null)) return null;

  const saved = data.month_uncached_cost_usd - data.month_cost_usd;
  const shown = view === 'cached' ? data.month_cost_usd : data.month_uncached_cost_usd;
  const low = Math.min(data.month_cost_usd, data.month_uncached_cost_usd);
  const high = Math.max(data.month_cost_usd, data.month_uncached_cost_usd);
  const breakEven = data.break_even_hit_ratio;

  return (
    <div className="pcard-section-panel">
      <div className="pcard-section-head">
        <span className="pcard-section-title">Caché</span>
        <div className="pcard-cost-pills pcard-cache-toggle" role="group" aria-label="Mostrar este mes con o sin caché">
          {(['cached', 'uncached'] as const).map((v) => (
            <button
              key={v}
              type="button"
              className={`pcard-cost-chip pcard-cache-toggle-btn${view === v ? ' is-active' : ''}`}
              aria-pressed={view === v}
              onClick={() => setView(v)}
            >
              {v === 'cached' ? 'Con caché' : 'Sin caché'}
            </button>
          ))}
        </div>
      </div>

      <div className="pcard-cache-month">
        <span className="pcard-cache-amount">{formatUsd(shown)}</span>
        <span className="pcard-cache-sub">
          este mes {view === 'uncached' ? 'sin caché' : 'con caché'} · rango {formatUsd(low)}–{formatUsd(high)}
        </span>
      </div>

      <div className="pcard-cache-stats">
        <div
          className="pcard-cache-stat"
          title="Costo de este mes sin caché menos el costo real, incluido el recargo por escritura"
        >
          <span className="pcard-cache-stat-k">{saved >= 0 ? 'Ahorro este mes' : 'Sobrecosto por caché'}</span>
          <span className="pcard-cache-stat-v">{formatUsd(Math.abs(saved))}</span>
        </div>
        <div className="pcard-cache-stat" title="Lecturas de caché sobre el total de tokens de prompt, últimos 7 días">
          <span className="pcard-cache-stat-k">Aciertos 7 días</span>
          <span className="pcard-cache-stat-v">{data.hit_ratio_7d != null ? pct(data.hit_ratio_7d) : '—'}</span>
        </div>
      </div>

      {breakEven != null && (
        <span className="pcard-mix-hint" title="Supone que cada fallo de caché se cobra como escritura en caché">
          {breakEven > 0 ? `La caché conviene sobre ${pct(breakEven)} de aciertos` : 'La caché siempre conviene · sin recargo de escritura'}
        </span>
      )}

      {projection != null && (
        <label className="pcard-cache-slider">
          <span className="pcard-cache-slider-label">Suponer {pct(hit)}</span>
          <span className="pcard-cache-track">
            <input
              type="range"
              min={0}
              max={100}
              step={1}
              value={Math.round(hit * 100)}
              onChange={(e) => setAssumed(Number(e.target.value) / 100)}
              aria-label="Tasa de aciertos de caché supuesta"
              style={{ ['--fill' as string]: `${hit * 100}%` }}
            />
            {breakEven != null && breakEven > 0 && (
              <span className="pcard-cache-tick" style={{ left: `${breakEven * 100}%` }} title={`Punto de equilibrio ${pct(breakEven)}`} />
            )}
          </span>
          <span
            className="pcard-cache-slider-value"
            title="Ahorro mensual al ritmo promedio de los últimos 7 días y la tasa de aciertos supuesta, suponiendo que cada fallo es una escritura en caché"
          >
            ≈ {savingLabel(projection)}/mes
          </span>
        </label>
      )}
    </div>
  );
};

export default CacheInsights;
