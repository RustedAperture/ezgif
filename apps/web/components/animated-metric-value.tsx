"use client";

import { animate, motion, useReducedMotion } from "framer-motion";
import { useEffect, useState } from "react";

export interface AnimatedMetricValueProps {
  value: number | null;
  formatValue: (value: number) => string;
  duration?: number;
  className?: string;
}

export function AnimatedMetricValue({
  value,
  formatValue,
  duration = 2,
  className,
}: AnimatedMetricValueProps) {
  const shouldReduceMotion = useReducedMotion();
  const [displayValue, setDisplayValue] = useState(0);

  useEffect(() => {
    if (value == null || shouldReduceMotion) {
      setDisplayValue(value ?? 0);
      return;
    }

    const controls = animate(0, value, {
      duration,
      ease: [0.16, 1, 0.3, 1],
      onUpdate: (latest) => setDisplayValue(Math.round(latest)),
    });

    return () => controls.stop();
  }, [duration, shouldReduceMotion, value]);

  return (
    <>
      <motion.span className={className} aria-hidden="true">
        {value == null ? "—" : formatValue(displayValue)}
      </motion.span>
      <span className="sr-only" role="status" aria-live="polite" aria-atomic="true">
        {value == null ? "—" : formatValue(value)}
      </span>
    </>
  );
}
