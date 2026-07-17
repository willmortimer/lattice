export interface ResourceLoadTicket {
  controller: AbortController;
  generation: number;
}

export interface ResourceLoadGate {
  begin: () => ResourceLoadTicket;
  isCurrent: (ticket: ResourceLoadTicket) => boolean;
  cancel: () => void;
}

export function createResourceLoadGate(): ResourceLoadGate {
  let generation = 0;
  let current: AbortController | null = null;
  return {
    begin: () => {
      current?.abort();
      const controller = new AbortController();
      current = controller;
      return { controller, generation: ++generation };
    },
    isCurrent: (ticket) => !ticket.controller.signal.aborted && ticket.generation === generation,
    cancel: () => {
      current?.abort();
      current = null;
      generation += 1;
    },
  };
}
