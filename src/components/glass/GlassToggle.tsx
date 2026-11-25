type GlassToggleProps = {
  checked: boolean;
  onChange: (next: boolean) => void;
  label?: string;
  // Kept for API compatibility; the switch is always ink (DESIGN.md: data and
  // controls are never colour-coded).
  color?: string;
};

/** On/off switch: ink pill when on, rail when off. Styling in design-system.css. */
const GlassToggle = ({ checked, onChange, label = 'Toggle' }: GlassToggleProps) => (
  <button
    type="button"
    className="qp-switch"
    onClick={() => onChange(!checked)}
    aria-pressed={checked}
    aria-label={label}
  >
    <span>{label}</span>
    <span className="qp-switch-track" aria-hidden="true">
      <span className="qp-switch-knob" />
    </span>
  </button>
);

export default GlassToggle;
