/**
 * Menghasilkan markup string SVG untuk marker foto drone.
 *
 * Fungsi murni deterministik tanpa dependensi React maupun Leaflet DOM.
 */
export function createDroneMarkerSvg(
  headingDeg: number,
  isActive: boolean,
  opacity: number = 1,
): string {
  return `
  <svg
    width="${isActive ? 36 : 24}"
    height="${isActive ? 36 : 24}"
    viewBox="0 0 24 24"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    style="transform: rotate(${headingDeg}deg); transition: all 0.3s; opacity: ${opacity};"
  >
    ${isActive ? '<circle cx="12" cy="12" r="12" fill="rgba(255, 165, 0, 0.4)"/>' : ""}
    <circle
      cx="12"
      cy="12"
      r="10"
      fill="${isActive ? "#FFA500" : "#2B95D6"}"
      stroke="white"
      stroke-width="2"
    />
    <path d="M12 4L16 14H8L12 4Z" fill="white"/>
  </svg>
`;
}
