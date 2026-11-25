import { ReactElement, ReactNode, SVGProps } from 'react';

/*
 * QuotaPulse interface icons (DESIGN.md → Iconography).
 * One hand-drawn set on a 16 px grid: 1.5 px stroke, round caps and joins,
 * currentColor, no fills except tiny dots. Replaces lucide-react.
 *
 * The props mirror the subset of the lucide API the app used (size,
 * className, strokeWidth), so call sites only change their import.
 */

export type IconProps = Omit<SVGProps<SVGSVGElement>, 'children'> & {
  size?: number;
  strokeWidth?: number;
};

export type IconComponent = ((props: IconProps) => ReactElement) & { displayName?: string };

const make = (name: string, body: ReactNode): IconComponent => {
  const Icon = ({ size = 16, strokeWidth = 1.5, className, ...rest }: IconProps) => (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      className={['qp-icon', className].filter(Boolean).join(' ')}
      {...rest}
    >
      {body}
    </svg>
  );
  Icon.displayName = name;
  return Icon;
};

const Dot = ({ cx, cy, r = 0.8 }: { cx: number; cy: number; r?: number }) => (
  <circle cx={cx} cy={cy} r={r} fill="currentColor" stroke="none" />
);

export const ChevronLeft = make('ChevronLeft', <path d="M10 3.5 5.5 8l4.5 4.5" />);
export const ChevronRight = make('ChevronRight', <path d="M6 3.5 10.5 8 6 12.5" />);

export const Home = make(
  'Home',
  <>
    <path d="M2.5 7 8 2.5 13.5 7" />
    <path d="M4 6v7.5h8V6" />
    <path d="M6.75 13.5V10.5h2.5v3" />
  </>,
);

export const Projects = make(
  'Projects',
  <>
    <path d="M2 4.5a1 1 0 0 1 1-1h3l1.5 1.5H13a1 1 0 0 1 1 1v6.5a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z" />
    <path d="M5.5 8v3M8 8v1.5M10.5 8v2.5" />
  </>,
);

export const Compare = make(
  'Compare',
  <>
    <path d="M3 5.5h9.5M10 3l2.5 2.5L10 8" />
    <path d="M13 10.5H3.5M6 8l-2.5 2.5L6 13" />
  </>,
);

export const Monitor = make(
  'Monitor',
  <>
    <rect x="2" y="2.5" width="12" height="8.5" rx="1.5" />
    <path d="M5.5 13.5h5M8 11v2.5" />
  </>,
);

export const Sun = make(
  'Sun',
  <>
    <circle cx="8" cy="8" r="2.75" />
    <path d="M8 1.5V3M8 13v1.5M1.5 8H3M13 8h1.5M3.4 3.4l1.06 1.06M11.54 11.54l1.06 1.06M3.4 12.6l1.06-1.06M11.54 4.46l1.06-1.06" />
  </>,
);

export const Moon = make('Moon', <path d="M13 9.6A5.5 5.5 0 1 1 6.4 3a4.4 4.4 0 0 0 6.6 6.6z" />);

export const Refresh = make(
  'Refresh',
  <>
    <path d="M13 2.75v3.5H9.5" />
    <path d="M12.6 6.25A5 5 0 0 0 3.2 6.5" />
    <path d="M3 13.25v-3.5h3.5" />
    <path d="M3.4 9.75a5 5 0 0 0 9.4-.25" />
  </>,
);

export const Sliders = make(
  'Sliders',
  <>
    <path d="M2.5 5h6M11.5 5h2" />
    <circle cx="10" cy="5" r="1.5" />
    <path d="M2.5 11h2M7.5 11h6" />
    <circle cx="6" cy="11" r="1.5" />
  </>,
);

export const Gauge = make(
  'Gauge',
  <>
    <path d="M2.5 11.5a5.5 5.5 0 1 1 11 0" />
    <path d="M8 11.5 10.6 8" />
    <Dot cx={8} cy={11.5} r={1} />
  </>,
);

export const History = make(
  'History',
  <>
    <path d="M2.6 8a5.4 5.4 0 1 0 1.6-3.85" />
    <path d="M2.5 2.5v3h3" />
    <path d="M8 5.25V8l2 1.5" />
  </>,
);

export const LogIn = make(
  'LogIn',
  <>
    <path d="M9.5 2.5H12a1.5 1.5 0 0 1 1.5 1.5v8a1.5 1.5 0 0 1-1.5 1.5H9.5" />
    <path d="M2.5 8h7M7 5.5 9.5 8 7 10.5" />
  </>,
);

export const Activity = make('Activity', <path d="M1.5 8h3l2-4.5 3 9 2-4.5h3" />);

export const Chip = make(
  'Chip',
  <>
    <rect x="4" y="4" width="8" height="8" rx="1.5" />
    <rect x="6.5" y="6.5" width="3" height="3" rx="0.5" />
    <path d="M6 2v2M10 2v2M6 12v2M10 12v2M2 6h2M2 10h2M12 6h2M12 10h2" />
  </>,
);

export const FolderOpen = make(
  'FolderOpen',
  <>
    <path d="M2 12.5v-8a1 1 0 0 1 1-1h3l1.5 1.5H12a1 1 0 0 1 1 1V7" />
    <path d="M2 12.5 3.8 8a1 1 0 0 1 .93-.6h9.07a.7.7 0 0 1 .64 1l-1.7 3.6a1 1 0 0 1-.9.5z" />
  </>,
);

/** Opens an external page (the project repository). */
export const ExternalLink = make(
  'ExternalLink',
  <>
    <path d="M9 2.5h4.5V7M13.5 2.5 7.5 8.5" />
    <path d="M12 9.5v3a1 1 0 0 1-1 1H3.5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1h3" />
  </>,
);

export const Layout = make(
  'Layout',
  <>
    <rect x="2.5" y="2.5" width="4.5" height="5.5" rx="1" />
    <rect x="9" y="2.5" width="4.5" height="3" rx="1" />
    <rect x="9" y="7.5" width="4.5" height="6" rx="1" />
    <rect x="2.5" y="10" width="4.5" height="3.5" rx="1" />
  </>,
);

export const Package = make(
  'Package',
  <>
    <path d="M8 1.75 13.5 4.5v7L8 14.25 2.5 11.5v-7z" />
    <path d="M2.5 4.5 8 7.25l5.5-2.75M8 7.25v7" />
  </>,
);

export const Bell = make(
  'Bell',
  <>
    <path d="M4 11.25V7.5a4 4 0 0 1 8 0v3.75l1.25 1.25H2.75z" />
    <path d="M6.5 14.25a1.6 1.6 0 0 0 3 0" />
  </>,
);

export const Download = make('Download', <path d="M8 2.5V10M5 7l3 3 3-3M3 13.5h10" />);

export const Tray = make(
  'Tray',
  <>
    <rect x="2" y="2.5" width="12" height="11" rx="2" />
    <path d="M5 6h6M5 8.5h6M5 11h3.5" />
  </>,
);

export const Palette = make(
  'Palette',
  <>
    <path d="M8 2a6 6 0 1 0 0 12c.9 0 1.3-.6 1.3-1.2 0-.9-.8-1.1-.8-1.9 0-.6.5-1 1.1-1H11a3 3 0 0 0 3-3C14 4.4 11.3 2 8 2z" />
    <Dot cx={5} cy={7.5} r={0.85} />
    <Dot cx={6.6} cy={4.8} r={0.85} />
    <Dot cx={9.8} cy={4.8} r={0.85} />
  </>,
);

export const Power = make(
  'Power',
  <>
    <path d="M8 2v5.5" />
    <path d="M4.6 4.2a5 5 0 1 0 6.8 0" />
  </>,
);

export const Server = make(
  'Server',
  <>
    <rect x="2.5" y="2.5" width="11" height="4.5" rx="1.25" />
    <rect x="2.5" y="9" width="11" height="4.5" rx="1.25" />
    <Dot cx={5} cy={4.75} r={0.75} />
    <Dot cx={5} cy={11.25} r={0.75} />
  </>,
);

export const Wallet = make(
  'Wallet',
  <>
    <path d="M2.5 4.75A1.75 1.75 0 0 1 4.25 3h7.25v1.75" />
    <rect x="2.5" y="4.75" width="11" height="8.75" rx="1.5" />
    <path d="M13.5 8H11a1 1 0 0 0 0 2h2.5" />
  </>,
);

export const Inbox = make(
  'Inbox',
  <>
    <path d="M2.5 9 4 3.5h8L13.5 9v3.5a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1z" />
    <path d="M2.5 9h3l1 1.5h3l1-1.5h3" />
  </>,
);

export const Alert = make(
  'Alert',
  <>
    <path d="M8 2.5 14 13H2z" />
    <path d="M8 6.5v3" />
    <Dot cx={8} cy={11.25} r={0.8} />
  </>,
);
