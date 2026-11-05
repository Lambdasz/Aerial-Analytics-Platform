import type { ParameterDefinition, ValidationResult } from "../types/plugin";

function isEmptyVal(val: unknown): boolean {
  return val === undefined || val === null || (typeof val === "string" && val.trim() === "");
}

export function validateParams(
  defs: ParameterDefinition[],
  values: Record<string, unknown>,
): ValidationResult<Record<string, unknown>> {
  const errors: Record<string, string> = {};
  const sanitized: Record<string, unknown> = {};

  for (const def of defs) {
    const rawVal = values[def.name] !== undefined ? values[def.name] : def.default;

    switch (def.type) {
      case "number": {
        const valToParse = isEmptyVal(rawVal) ? def.default : rawVal;

        if (
          isEmptyVal(valToParse) ||
          typeof valToParse === "boolean" ||
          Array.isArray(valToParse)
        ) {
          errors[def.name] = `${def.label} must be a valid number.`;
          break;
        }

        const num =
          typeof valToParse === "number"
            ? valToParse
            : Number(typeof valToParse === "string" ? valToParse.trim() : valToParse);

        if (!Number.isFinite(num)) {
          errors[def.name] = `${def.label} must be a valid number.`;
        } else if (def.min !== undefined && num < def.min) {
          errors[def.name] = `${def.label} cannot be less than ${def.min}.`;
        } else if (def.max !== undefined && num > def.max) {
          errors[def.name] = `${def.label} cannot exceed ${def.max}.`;
        } else if (def.step !== undefined && def.step > 0) {
          const ratio = Math.abs(num - (def.min ?? 0)) / def.step;
          if (Math.abs(ratio - Math.round(ratio)) >= 1e-5) {
            errors[def.name] = `${def.label} must be a valid multiple of step ${def.step}.`;
          } else {
            sanitized[def.name] = num;
          }
        } else {
          sanitized[def.name] = num;
        }
        break;
      }
      case "select": {
        const fallback =
          (rawVal === null ||
            rawVal === undefined ||
            (rawVal === "" && def.options && !def.options.includes(""))) &&
          def.default !== undefined
            ? def.default
            : rawVal;
        const strVal =
          typeof fallback === "string"
            ? fallback
            : typeof fallback === "number" || typeof fallback === "boolean"
              ? fallback.toString()
              : "";
        if (def.options && !def.options.includes(strVal)) {
          errors[def.name] = `Invalid option selected for ${def.label}.`;
        } else {
          sanitized[def.name] = strVal;
        }
        break;
      }
      case "boolean": {
        const valToUse =
          (rawVal === undefined || rawVal === null || rawVal === "") && def.default !== undefined
            ? def.default
            : rawVal;
        sanitized[def.name] = valToUse === true || valToUse === "true";
        break;
      }
      case "string": {
        sanitized[def.name] =
          typeof rawVal === "string"
            ? rawVal
            : typeof rawVal === "number" || typeof rawVal === "boolean"
              ? rawVal.toString()
              : "";
        break;
      }
      default: {
        sanitized[def.name] = rawVal;
        break;
      }
    }
  }

  if (Object.keys(errors).length > 0) {
    return { isValid: false, errors };
  }
  return { isValid: true, data: sanitized };
}
