import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  BookOpen,
  Box,
  Cloud,
  Database,
  Download,
  FileText,
  Globe,
  HardDrive,
  LayoutGrid,
  Monitor,
  Puzzle,
  Radio,
  Server,
  Video,
} from "lucide-react";

const ICONS = {
  box: Box,
  globe: Globe,
  server: Server,
  video: Video,
  database: Database,
  fileText: FileText,
  cloud: Cloud,
  hardDrive: HardDrive,
  layoutGrid: LayoutGrid,
  bookOpen: BookOpen,
  download: Download,
  radio: Radio,
  monitor: Monitor,
  puzzle: Puzzle,
} as const;

export const ICON_OPTIONS = Object.keys(ICONS);

const cache = new Map<string, Promise<string | null>>();

function loadFavicon(url: string): Promise<string | null> {
  let hit = cache.get(url);
  if (!hit) {
    hit = invoke<string | null>("fetch_nav_icon", { url }).catch(() => null);
    cache.set(url, hit);
  }
  return hit;
}

export function NavIcon({
  name,
  url,
  size = 20,
}: {
  name: string;
  url?: string;
  size?: number;
}) {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    if (!url) {
      setSrc(null);
      return;
    }
    let live = true;
    setSrc(null);
    void loadFavicon(url).then((next) => {
      if (live) setSrc(next);
    });
    return () => {
      live = false;
    };
  }, [url]);

  if (src) {
    return (
      <img
        className="nav-favicon"
        src={src}
        width={size}
        height={size}
        alt=""
        draggable={false}
        style={{ width: size, height: size }}
      />
    );
  }

  const Icon = ICONS[name as keyof typeof ICONS] ?? Box;
  return <Icon size={size} />;
}
