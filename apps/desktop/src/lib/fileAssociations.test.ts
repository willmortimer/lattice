import { describe, expect, it } from "vitest";
import plistText from "../../src-tauri/Info.plist?raw";
import configText from "../../src-tauri/tauri.conf.json?raw";

type FileAssociation = {
  ext: string[];
  name: string;
  role: string;
  rank?: string;
  mimeType?: string;
  contentTypes?: string[];
};

type PlistDocumentType = {
  name: string;
  role: string;
  rank: string;
  extensions: string[];
  contentTypes: string[];
};

const VIEWER_IMAGE_EXTS = [
  "png",
  "jpg",
  "jpeg",
  "gif",
  "webp",
  "avif",
  "bmp",
  "tif",
  "tiff",
] as const;

const IMAGE_UT_TYPES = [
  "public.png",
  "public.jpeg",
  "public.gif",
  "org.webmproject.webp",
  "public.avif",
  "com.microsoft.bmp",
  "public.tiff",
] as const;

function associationForExt(associations: FileAssociation[], ext: string): FileAssociation {
  const match = associations.find((entry) => entry.ext.includes(ext));
  expect(match, `missing fileAssociation for .${ext}`).toBeDefined();
  return match as FileAssociation;
}

function plistString(block: string, key: string): string {
  const match = block.match(new RegExp(`<key>${key}</key>\\s*<string>([^<]*)</string>`));
  return match?.[1] ?? "";
}

function plistStrings(block: string, key: string): string[] {
  const keyTag = `<key>${key}</key>`;
  const keyIdx = block.indexOf(keyTag);
  if (keyIdx < 0) return [];
  const arrayMatch = block.slice(keyIdx + keyTag.length).match(/<array>([\s\S]*?)<\/array>/);
  if (!arrayMatch) return [];
  return [...arrayMatch[1].matchAll(/<string>([^<]*)<\/string>/g)].map((match) => match[1]);
}

function parsePlistDocumentTypes(plist: string): PlistDocumentType[] {
  const key = "<key>CFBundleDocumentTypes</key>";
  const start = plist.indexOf(key);
  expect(start).toBeGreaterThanOrEqual(0);
  const after = plist.slice(start + key.length);
  const nextTopKey = after.search(/\n\t<key>/);
  const arrayXml = nextTopKey < 0 ? after : after.slice(0, nextTopKey);
  return [...arrayXml.matchAll(/<dict>([\s\S]*?)<\/dict>/g)].map((match) => {
    const block = match[1];
    return {
      name: plistString(block, "CFBundleTypeName"),
      role: plistString(block, "CFBundleTypeRole"),
      rank: plistString(block, "LSHandlerRank"),
      extensions: plistStrings(block, "CFBundleTypeExtensions"),
      contentTypes: plistStrings(block, "LSItemContentTypes"),
    };
  });
}

function documentTypeForExt(types: PlistDocumentType[], ext: string): PlistDocumentType {
  const match = types.find((entry) => entry.extensions.includes(ext));
  expect(match, `missing CFBundleDocumentTypes entry for .${ext}`).toBeDefined();
  return match as PlistDocumentType;
}

describe("desktop file associations", () => {
  const config = JSON.parse(configText) as {
    bundle: { fileAssociations: FileAssociation[] };
  };
  const associations = config.bundle.fileAssociations;
  const documentTypes = parsePlistDocumentTypes(plistText);

  it("keeps Markdown as Editor, notebooks as Owner, and CSV as Viewer Alternate", () => {
    for (const ext of ["md", "markdown", "mdown"]) {
      expect(associationForExt(associations, ext).role).toBe("Editor");
      const type = documentTypeForExt(documentTypes, ext);
      expect(type.role).toBe("Editor");
      expect(type.rank).toBe("Default");
    }

    expect(associationForExt(associations, "ipynb").role).toBe("Editor");
    const notebook = documentTypeForExt(documentTypes, "ipynb");
    expect(notebook.role).toBe("Editor");
    expect(notebook.rank).toBe("Owner");

    for (const ext of ["csv", "tsv"]) {
      expect(associationForExt(associations, ext).role).toBe("Viewer");
      const type = documentTypeForExt(documentTypes, ext);
      expect(type.role).toBe("Viewer");
      expect(type.rank).toBe("Alternate");
    }
  });

  it("registers PDF as a Viewer Alternate, not Owner", () => {
    const json = associationForExt(associations, "pdf");
    expect(json.role).toBe("Viewer");
    expect(json.rank).toBe("Alternate");
    expect(json.mimeType).toBe("application/pdf");
    expect(json.contentTypes).toContain("com.adobe.pdf");

    const type = documentTypeForExt(documentTypes, "pdf");
    expect(type.role).toBe("Viewer");
    expect(type.rank).toBe("Alternate");
    expect(type.contentTypes).toContain("com.adobe.pdf");
  });

  it("registers common images as Viewer Alternate with system UTTypes", () => {
    for (const ext of VIEWER_IMAGE_EXTS) {
      const json = associationForExt(associations, ext);
      expect(json.role).toBe("Viewer");
      expect(json.rank).toBe("Alternate");

      const type = documentTypeForExt(documentTypes, ext);
      expect(type.role).toBe("Viewer");
      expect(type.rank).toBe("Alternate");
    }

    const imageJson = associationForExt(associations, "png");
    const imageType = documentTypeForExt(documentTypes, "png");
    for (const utType of IMAGE_UT_TYPES) {
      expect(imageJson.contentTypes).toContain(utType);
      expect(imageType.contentTypes).toContain(utType);
    }
  });

  it("does not declare NSServices (that is a later slice)", () => {
    expect(plistText).not.toContain("NSServices");
  });
});
