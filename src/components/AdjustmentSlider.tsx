interface AdjustmentSliderProps {
  /** DOM id / label `htmlFor` target. */
  id: string;
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  /** Neutral value; the reset button shows only when `value` differs from it. */
  defaultValue: number;
  disabled: boolean;
  onChange: (value: number) => void;
  /** Render the numeric readout (e.g. "+1.20 EV", "-30"). */
  format: (value: number) => string;
}

/**
 * One adjustment row, wired through the shared adjustment definition. Generic so
 * every Phase 2 operation is just a definition field + a shader line + one of
 * these — no bespoke slider per control.
 */
export function AdjustmentSlider({
  id,
  label,
  value,
  min,
  max,
  step,
  defaultValue,
  disabled,
  onChange,
  format,
}: AdjustmentSliderProps) {
  return (
    <div className="control">
      <div className="control__head">
        <label htmlFor={id}>{label}</label>
        <div className="control__value">
          <span>{format(value)}</span>
          {value !== defaultValue && (
            <button
              className="control__reset"
              onClick={() => onChange(defaultValue)}
              disabled={disabled}
              title={`Reset ${label.toLowerCase()}`}
            >
              Reset
            </button>
          )}
        </div>
      </div>
      <input
        id={id}
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(parseFloat(e.target.value))}
      />
    </div>
  );
}
