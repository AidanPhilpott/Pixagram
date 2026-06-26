import { EXPOSURE_RANGE } from "../adjustments/definition";

interface ExposureSliderProps {
  value: number;
  disabled: boolean;
  onChange: (ev: number) => void;
}

/** The single Phase 1 slider, wired through the shared adjustment definition. */
export function ExposureSlider({ value, disabled, onChange }: ExposureSliderProps) {
  return (
    <div className="control">
      <div className="control__head">
        <label htmlFor="exposure">Exposure</label>
        <div className="control__value">
          <span>{value >= 0 ? `+${value.toFixed(2)}` : value.toFixed(2)} EV</span>
          {value !== 0 && (
            <button
              className="control__reset"
              onClick={() => onChange(0)}
              disabled={disabled}
              title="Reset exposure"
            >
              Reset
            </button>
          )}
        </div>
      </div>
      <input
        id="exposure"
        type="range"
        min={EXPOSURE_RANGE.min}
        max={EXPOSURE_RANGE.max}
        step={EXPOSURE_RANGE.step}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(parseFloat(e.target.value))}
      />
    </div>
  );
}
