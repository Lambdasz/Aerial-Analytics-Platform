import type { ParameterDefinition, ValidationResult } from "../types/plugin";

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
        const num = Number(rawVal);
        if (Number.isNaN(num)) {
          errors[def.name] = `${def.label} must be a valid number.`;
        } else if (def.min !== undefined && num < def.min) {
          errors[def.name] = `${def.label} cannot be less than ${def.min}.`;
        } else if (def.max !== undefined && num > def.max) {
          errors[def.name] = `${def.label} cannot exceed ${def.max}.`;
        } else {
          sanitized[def.name] = num;
        }
        break;
      }
      case "select": {
        const strVal =
          typeof rawVal === "string"
            ? rawVal
            : typeof rawVal === "number" || typeof rawVal === "boolean"
              ? rawVal.toString()
              : "";
        if (def.options && !def.options.includes(strVal)) {
          errors[def.name] = `Invalid option selected for ${def.label}.`;
        } else {
          sanitized[def.name] = strVal;
        }
        break;
      }
      case "boolean": {
        sanitized[def.name] = Boolean(rawVal);
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
