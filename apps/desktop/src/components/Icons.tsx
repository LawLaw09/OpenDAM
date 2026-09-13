// Minimal SVG icon set for OpenDAM
// Each icon is a functional component accepting className/size props

interface IconProps {
  className?: string;
  size?: number;
}

const icon = (path: string) =>
  function Icon({ className, size = 16 }: IconProps) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={2}
        strokeLinecap="round"
        strokeLinejoin="round"
        className={className}
        aria-hidden
      >
        <path d={path} />
      </svg>
    );
  };

export const IconGrid = icon('M3 3h7v7H3zM14 3h7v7h-7zM3 14h7v7H3zM14 14h7v7h-7z');
export const IconList = icon('M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01');
export const IconSearch = icon('m21 21-4.35-4.35M17 11A6 6 0 1 1 5 11a6 6 0 0 1 12 0z');
export const IconSidebar = icon('M3 3h18v18H3zM9 3v18');
export const IconPanel = icon('M3 3h18v18H3zM15 3v18');
export const IconRefresh = icon('M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15');
export const IconPlus = icon('M12 5v14M5 12h14');
export const IconStar = icon('M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z');
export const IconTag = icon('M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82zM7 7h.01');
export const IconFolder = icon('M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z');
export const IconCollection = icon('M19 11H5m14 0a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-6a2 2 0 0 1 2-2m14 0V9a2 2 0 0 0-2-2M5 11V9a2 2 0 0 1 2-2m0 0V5a2 2 0 0 1 2-2h6a2 2 0 0 1 2 2v2M7 7h10');
export const IconChevronRight = icon('M9 18l6-6-6-6');
export const IconChevronDown = icon('M6 9l6 6 6-6');
export const IconEye = icon('M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8zM12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z');
export const IconInfo = icon('M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20zM12 8v4M12 16h.01');
export const IconFilter = icon('M22 3H2l8 9.46V19l4 2v-8.54L22 3z');
export const IconX = icon('M18 6 6 18M6 6l12 12');
export const IconCheck = icon('M20 6 9 17l-5-5');
export const IconDots = icon('M12 5h.01M12 12h.01M12 19h.01');
export const IconCopy = icon('M20 9h-9a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h9a2 2 0 0 0 2-2v-9a2 2 0 0 0-2-2z M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1');
export const IconExternal = icon('M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6M15 3h6v6M10 14 21 3');
export const IconTrash = icon('M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2');
export const IconModel3D = icon('M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z');
export const IconImage = icon('M21 19V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2zM8.5 14l2.5-3 2.5 3 2-2.4 2.5 3H6z');
export const IconVideo = icon('M15 10l4.553-2.277A1 1 0 0 1 21 8.623v6.754a1 1 0 0 1-1.447.896L15 14v-4zM3 8a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8z');
export const IconLayers = icon('M12 2L2 7l10 5 10-5-10-5z M2 17l10 5 10-5 M2 12l10 5 10-5');
export const IconFileText = icon('M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z M14 2v6h6');
export const IconInbox = icon('M22 12h-6l-2 3h-4l-2-3H2v8a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-8z M5.45 5.11L2 12v0a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v0l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z');

