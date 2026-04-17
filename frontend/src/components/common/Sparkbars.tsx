interface SparkbarsProps {
  values: number[];
  width?: number;
  height?: number;
  fill?: string;
  emptyFill?: string;
  label?: string;
}

export default function Sparkbars({
  values,
  width = 60,
  height = 16,
  fill = "#1A1A1A",
  emptyFill = "#E5E7EB",
  label,
}: SparkbarsProps) {
  const count = values.length || 5;
  const gap = 1;
  const barWidth = Math.max(1, Math.floor((width - gap * (count - 1)) / count));
  const max = Math.max(1, ...values);

  return (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      aria-label={label ?? "sparkline"}
      role="img"
      shapeRendering="crispEdges"
    >
      {values.length === 0
        ? Array.from({ length: count }).map((_, i) => (
            <rect
              key={i}
              x={i * (barWidth + gap)}
              y={height - 2}
              width={barWidth}
              height={2}
              fill={emptyFill}
            />
          ))
        : values.map((v, i) => {
            const h = Math.max(1, Math.round((v / max) * (height - 1)));
            return (
              <rect
                key={i}
                x={i * (barWidth + gap)}
                y={height - h}
                width={barWidth}
                height={h}
                fill={fill}
              />
            );
          })}
    </svg>
  );
}
