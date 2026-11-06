import { Database, FileCode2, Globe2 } from "lucide-react";
import { ServiceType } from "../types/services";

export const SERVICE_ICONS = {
  [ServiceType.Caddy]: Globe2,
  [ServiceType.PhpFpm]: FileCode2,
  [ServiceType.MySQL]: Database,
  [ServiceType.PostgreSQL]: Database,
};
