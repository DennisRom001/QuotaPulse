import { ButtonHTMLAttributes, PropsWithChildren } from 'react';

type Variant = 'default' | 'primary' | 'danger';
type Size = 'sm' | 'md';

type GlassButtonProps = PropsWithChildren<
  Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'color'> & {
    variant?: Variant;
    size?: Size;
    providerColor?: string;
  }
>;

const sizeStyle: Record<Size, { padding: string; fontSize: number }> = {
  sm: { padding: '5px 10px', fontSize: 11 },
  md: { padding: '7px 12px', fontSize: 12 },
};

const GlassButton = ({
  children,
  variant = 'default',
  size = 'md',
  providerColor,
  disabled,
  className = '',
  style,
  ...props
}: GlassButtonProps) => {
  // Pill buttons: primary = solid ink, danger = red outline, default = quiet
  // sheet. Providers are never colour-coded, so providerColor is ignored.
  void providerColor;
  const tone =
    variant === 'primary'
      ? { background: 'var(--qp-ink)', borderColor: 'var(--qp-ink)', color: 'var(--qp-on-ink)' }
      : variant === 'danger'
        ? { background: 'transparent', borderColor: 'var(--qp-danger)', color: 'var(--qp-danger)' }
        : {};

  return (
    <button
      type="button"
      className={`glass-pill transition-smooth ${className}`.trim()}
      disabled={disabled}
      style={{
        ...sizeStyle[size],
        cursor: disabled ? 'not-allowed' : 'pointer',
        opacity: disabled ? 0.4 : 1,
        pointerEvents: disabled ? 'none' : undefined,
        borderRadius: 999,
        fontWeight: 600,
        ...tone,
        ...style,
      }}
      {...props}
    >
      {children}
    </button>
  );
};

export default GlassButton;

