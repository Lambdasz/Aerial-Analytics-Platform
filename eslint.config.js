import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import jsxA11y from "eslint-plugin-jsx-a11y";
import eslintConfigPrettier from "eslint-config-prettier";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist", "src-tauri/target"] },
  {
    files: ["**/*.{ts,tsx}"],
    extends: [
      js.configs.recommended,
      ...tseslint.configs.recommendedTypeChecked,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
      jsxA11y.flatConfigs.recommended,
    ],
    languageOptions: {
      ecmaVersion: 2020,
      globals: globals.browser,
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
  {
    files: ["src/modules/module-03/atoms/**/*.{ts,tsx}"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: [
                "**/molecules/**",
                "**/organisms/**",
                "**/templates/**",
                "**/hooks/**",
                "**/store/**",
                "**/services/**",
                "**/lib/leaflet/**",
              ],
              message:
                "Atoms tidak boleh mengimpor molecules, organisms, templates, hooks, store, services, atau lib/leaflet.",
            },
          ],
          paths: [
            {
              name: "react-leaflet",
              importNames: ["useMap", "useMapEvent", "useMapEvents"],
              message: "Atoms dilarang mengonsumsi hook Leaflet MapContext.",
            },
          ],
        },
      ],
    },
  },
  {
    files: ["src/modules/module-03/molecules/**/*.{ts,tsx}"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: [
                "**/organisms/**",
                "**/templates/**",
                "**/hooks/**",
                "**/store/**",
                "**/services/**",
                "**/lib/leaflet/**",
              ],
              message:
                "Molecules tidak boleh mengimpor organisms, templates, hooks, store, services, atau lib/leaflet (wajib murni presentasional).",
            },
          ],
          paths: [
            {
              name: "react-leaflet",
              importNames: ["useMap", "useMapEvent", "useMapEvents"],
              message:
                "Molecules dilarang mengonsumsi hook Leaflet MapContext (gunakan Organisms jika memerlukan useMap).",
            },
          ],
        },
      ],
    },
  },
  {
    files: ["src/modules/module-03/lib/*.ts"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          paths: [
            { name: "react", message: "lib/ murni dilarang mengimpor React." },
            {
              name: "leaflet",
              message:
                "lib/ murni dilarang mengimpor Leaflet (gunakan lib/leaflet/ untuk builder Leaflet).",
            },
          ],
        },
      ],
    },
  },
  {
    files: ["src/modules/module-03/lib/leaflet/*.ts"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          paths: [{ name: "react", message: "lib/leaflet/ dilarang mengimpor React." }],
        },
      ],
    },
  },
  eslintConfigPrettier,
);
