import { useState } from "react";
import { AdjustmentSlider } from "./AdjustmentSlider";
import { HSL_BANDS, HSL_RANGE, type HslBand } from "../adjustments/definition";

interface HslPanelProps {
  bands: HslBand[];
  disabled: boolean;
  onChange: (index: number, field: keyof HslBand, value: number) => void;
}

const asPercent = (v: number) => `${v > 0 ? "+" : ""}${Math.round(v * 100)}`;
const isEdited = (b: HslBand | undefined) =>
  !!b && (b.hue !== 0 || b.sat !== 0 || b.lum !== 0);

/**
 * Per-hue HSL: pick a color band from the swatch row, then edit its hue,
 * saturation, and luminance. Compact enough for the narrow inspector.
 */
export function HslPanel({ bands, disabled, onChange }: HslPanelProps) {
  const [selected, setSelected] = useState(0);
  const band = bands[selected] ?? { hue: 0, sat: 0, lum: 0 };
  const axes: Array<{ field: keyof HslBand; label: string }> = [
    { field: "hue", label: "Hue" },
    { field: "sat", label: "Saturation" },
    { field: "lum", label: "Luminance" },
  ];

  return (
    <div className="hsl">
      <div className="hsl__head">
        <label>HSL / Color</label>
      </div>
      <div className="hsl__swatches" role="tablist" aria-label="HSL color band">
        {HSL_BANDS.map((b, i) => (
          <button
            key={b.name}
            type="button"
            role="tab"
            aria-selected={i === selected}
            className={`hsl__swatch${i === selected ? " hsl__swatch--active" : ""}${
              isEdited(bands[i]) ? " hsl__swatch--edited" : ""
            }`}
            style={{ background: `hsl(${b.hue}, 65%, 55%)` }}
            title={b.name}
            onClick={() => setSelected(i)}
          />
        ))}
      </div>
      <div className="hsl__band-label">{HSL_BANDS[selected].name}</div>
      {axes.map((axis) => (
        <AdjustmentSlider
          key={axis.field}
          id={`hsl-${axis.field}`}
          label={axis.label}
          value={band[axis.field]}
          min={HSL_RANGE.min}
          max={HSL_RANGE.max}
          step={HSL_RANGE.step}
          defaultValue={0}
          disabled={disabled}
          onChange={(v) => onChange(selected, axis.field, v)}
          format={asPercent}
        />
      ))}
    </div>
  );
}
