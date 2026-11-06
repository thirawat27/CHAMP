import { describe, expect, it } from "vitest";
import {
  EMPTY_PACKAGE_SELECTION,
  PackagesConfig,
  normalizeSelection,
  packagesForPlatform,
} from "./services";

const pkg = (id: string, windowsX64 = "https://example.test/x.zip") => ({
  id,
  version: id,
  display_name: id,
  windowsX64,
  windowsArm64: "",
  linuxX64: "",
  linuxArm64: "",
  macOSX64: "",
  macOSArm64: "",
  eol: false,
  lts: false,
  recommended: false,
});

const packages: PackagesConfig = {
  php: [pkg("php-8.5"), pkg("php-8.4", "")],
  mysql: [pkg("mysql-9")],
  postgresql: [pkg("pg-18")],
  phpmyadmin: [
    {
      id: "phpmyadmin-5",
      version: "5",
      display_name: "pma",
      url: "",
      eol: false,
      lts: false,
      recommended: false,
    },
  ],
};

describe("catalog helpers", () => {
  it("lists only packages with a download for the platform", () => {
    expect(packagesForPlatform(packages.php, "windowsX64").map((p) => p.id)).toEqual(["php-8.5"]);
    expect(packagesForPlatform(packages.php, "linuxX64")).toEqual([]);
  });

  it("moves unavailable ids to the first available package and keeps valid ones", () => {
    const result = normalizeSelection(
      { ...EMPTY_PACKAGE_SELECTION, php: "php-8.4", mysql: "mysql-9", database_engine: "mysql" },
      packages,
      "windowsX64"
    );
    expect(result).toEqual({
      php: "php-8.5",
      mysql: "mysql-9",
      postgresql: "pg-18",
      phpmyadmin: "phpmyadmin-5",
      database_engine: "mysql",
    });
  });

  it("keeps the id when the platform has no package at all", () => {
    const result = normalizeSelection(
      { ...EMPTY_PACKAGE_SELECTION, php: "php-8.5" },
      packages,
      "linuxX64"
    );
    expect(result.php).toBe("php-8.5");
  });
});
