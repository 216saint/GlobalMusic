// Atualização do app: o card "Atualização disponível" (UpdateCard) e o botão "Buscar atualizações"
// do Perfil usam o mesmo estado.

import { api, errorMessage } from "./api";
import type { UpdateInfo, UpdateProgress } from "./types";

class UpdateStore {
  update = $state<UpdateInfo | null>(null);
  dismissed = $state(false);
  checking = $state(false);
  installing = $state(false);
  progress = $state<UpdateProgress | null>(null);
  error = $state<string | null>(null);

  /**
   * Procura uma versão nova. Automática (`manual = false`): falhas ficam em silêncio, sem rede ou
   * com o servidor fora do ar não é motivo para incomodar. Pedida pela pessoa: devolve a resposta
   * em texto ("já está na mais recente", o erro…) ou `null` quando há versão nova (o card aparece).
   */
  async check(manual = false): Promise<string | null> {
    if (this.installing || this.checking) return null;
    this.checking = true;
    try {
      this.update = await api.checkUpdate();
      this.dismissed = false;
      return this.update ? null : "Você já está na versão mais recente.";
    } catch (e) {
      return manual ? `Não deu para verificar agora: ${errorMessage(e)}` : null;
    } finally {
      this.checking = false;
    }
  }

  async install() {
    this.installing = true;
    this.error = null;
    try {
      await api.installUpdate(); // não retorna: o app reinicia na versão nova
    } catch (e) {
      this.error = errorMessage(e);
      this.installing = false;
    }
  }
}

export const updates = new UpdateStore();
