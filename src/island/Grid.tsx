import { useEffect, useState } from "react";
import { motion } from "motion/react";
import { openSettings } from "../shared/settings";

/** Expanded home view. Only the clock is real in Phase 1; the other cards show which phase fills them. */
export default function Grid() {
  return (
    <motion.div className="grid" initial="hidden" animate="shown" variants={{ shown: { transition: { staggerChildren: 0.035 } } }}>
      <Card className="agent" title="Claude Code" detail="Agent status: phase 5" />
      <Card title="Music" detail="Phase 3" />
      <Card title="Focus" detail="Timer: phase 2" />
      <Clock />
      <Card title="Settings" detail="Preferences" onClick={openSettings} />
    </motion.div>
  );
}

const cardVariants = {
  hidden: { opacity: 0, y: 8, scale: 0.97 },
  shown: { opacity: 1, y: 0, scale: 1, transition: { type: "spring", stiffness: 500, damping: 32 } },
} as const;

type CardProps = { title: string; detail: string; className?: string; onClick?: () => void };

function Card({ title, detail, className = "", onClick }: CardProps) {
  return (
    <motion.div
      className={`card ${className} ${onClick ? "clickable" : ""}`}
      variants={cardVariants}
      onClick={onClick}
      whileHover={onClick ? { scale: 1.03 } : undefined}
      whileTap={onClick ? { scale: 0.97 } : undefined}
    >
      <span className="card-title">{title}</span>
      <span className="card-detail">{detail}</span>
    </motion.div>
  );
}

function Clock() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(timer);
  }, []);
  return (
    <motion.div className="card" variants={cardVariants}>
      <span className="clock-time">{now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
      <span className="card-detail">{now.toLocaleDateString([], { weekday: "short", day: "numeric", month: "short" })}</span>
    </motion.div>
  );
}
