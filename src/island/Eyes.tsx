import { motion } from "motion/react";

/**
 * Placeholder eyes until the real mascot design arrives.
 * Idle: calm, blinking every few seconds. Peek: eyes widen.
 */
export default function Eyes({ awake, size = 1 }: { awake: boolean; size?: number }) {
  return (
    <motion.div className="eyes" animate={{ scale: (awake ? 1.18 : 1) * size }} transition={SPRING}>
      <Eye />
      <Eye />
    </motion.div>
  );
}

function Eye() {
  return (
    <motion.span
      className="eye"
      animate={{ scaleY: [1, 1, 0.1, 1] }}
      transition={{ duration: 4, times: [0, 0.94, 0.97, 1], repeat: Infinity, ease: "easeInOut" }}
    />
  );
}

const SPRING = { type: "spring", stiffness: 420, damping: 22 } as const;
