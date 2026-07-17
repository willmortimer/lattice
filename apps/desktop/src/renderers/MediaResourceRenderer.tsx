import type { ResourceRendererProps } from "../resourceRendererRegistry";
import { deriveResourceFormatId } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererContext } from "./RendererContext";
import { ImageViewer } from "../viewers/media/ImageViewer";
import { PdfViewer } from "../viewers/media/PdfViewer";
import { MediaDegraded } from "../viewers/media/MediaDegraded";

export function MediaResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  const formatId = deriveResourceFormatId(session.resource);
  if (formatId === "pdf" || formatId === "file:pdf") return <PdfViewer context={context} resource={session.resource} />;
  if (formatId !== "image" && formatId !== "file:image") {
    return <FileResourceFallbackRenderer context={context} session={session} />;
  }
  return <ImageViewer context={context} resource={session.resource} />;
}

export function FileResourceFallbackRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  return (
    <MediaDegraded
      context={context}
      resource={session.resource}
      title="No built-in file viewer"
      message="This file remains available as canonical workspace content."
      detail={session.resource.path}
    />
  );
}
