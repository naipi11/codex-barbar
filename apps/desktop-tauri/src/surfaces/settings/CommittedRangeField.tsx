import { useState } from "react";
import { useCommittedRange } from "../../hooks/useCommittedRange";

export interface CommittedRangeFieldProps {
  id: string;
  label: string;
  value: number;
  min: number;
  max: number;
  tickValues: readonly number[];
  valueText(value: number): string;
  onCommit(next: number): Promise<number>;
  disabled?: boolean;
  errorMessage?: string;
  help?: string;
}

export function CommittedRangeField({
  id,
  label,
  value,
  min,
  max,
  tickValues,
  valueText,
  onCommit,
  disabled = false,
  errorMessage,
  help,
}: CommittedRangeFieldProps) {
  const [hasSaveError, setHasSaveError] = useState(false);
  const range = useCommittedRange({
    value,
    min,
    max,
    onCommit,
    onError: () => setHasSaveError(true),
    onSuccess: () => setHasSaveError(false),
  });

  return (
    <div className="settings-committed-range">
      <div className="settings-range">
        <label htmlFor={id}>{label}</label>
        <output>{valueText(range.value)}</output>
      </div>
      <input
        id={id}
        type="range"
        min={min}
        max={max}
        step="1"
        value={range.value}
        disabled={disabled}
        aria-label={label}
        aria-valuetext={valueText(range.value)}
        onChange={() => undefined}
        onInput={range.onInput}
        onPointerDown={range.onPointerDown}
        onPointerUp={range.onPointerUp}
        onPointerCancel={range.onPointerCancel}
        onKeyDown={range.onKeyDown}
        onKeyUp={range.onKeyUp}
        onBlur={range.onBlur}
      />
      <div className="settings-range__ticks" aria-hidden="true">
        {tickValues.map((tick) => <span key={tick}>{tick}</span>)}
      </div>
      {help ? <p className="settings-preference-group__hint">{help}</p> : null}
      {hasSaveError && errorMessage ? (
        <p className="settings-preference-group__error" role="alert">{errorMessage}</p>
      ) : null}
    </div>
  );
}
