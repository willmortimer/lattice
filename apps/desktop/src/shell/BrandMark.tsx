/** Generated from the same axonometric unit-cell geometry as the site mark. */
export function BrandMark({ size = 64 }: { size?: number }) {
  return (
    <svg
      className="brand-mark"
      width={size}
      height={size}
      viewBox="0 0 56 56"
      fill="none"
      aria-hidden="true"
    >
      <g stroke="var(--lt-accent)" strokeLinecap="round">
        <path d="M28 28L47.05 39M28 28L28 6M28 28L8.95 39" strokeWidth="1.7" opacity="0.28" />
        <path
          d="M37.53 11.5L18.47 22.5M18.47 11.5L37.53 22.5M47.05 28L28 39M37.53 44.5L37.53 22.5M18.47 44.5L18.47 22.5M8.95 28L28 39"
          strokeWidth="1.85"
          opacity="0.45"
        />
        <path
          d="M28 6L47.05 17M47.05 17L47.05 39M47.05 39L28 50M28 50L8.95 39M8.95 39L8.95 17M8.95 17L28 6"
          strokeWidth="2.45"
          opacity="0.9"
        />
        <path d="M28 28L47.05 17M28 28L8.95 17M28 28L28 50" strokeWidth="2.45" opacity="0.95" />
      </g>
      <g fill="var(--lt-accent-bright)">
        <circle cx="28" cy="6" r="2.75" />
        <circle cx="47.05" cy="17" r="2.75" />
        <circle cx="47.05" cy="39" r="2.75" />
        <circle cx="28" cy="50" r="2.75" />
        <circle cx="8.95" cy="39" r="2.75" />
        <circle cx="8.95" cy="17" r="2.75" />
      </g>
      <circle cx="28" cy="28" r="4.2" fill="var(--lt-accent)" />
    </svg>
  );
}
