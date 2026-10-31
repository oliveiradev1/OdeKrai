import { Fragment, useCallback, useEffect, useMemo, useState, type FormEvent } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { open, save as saveDialog } from '@tauri-apps/plugin-dialog';
import { PageHeader } from '../DesignSystem';
import { PetSprite } from '../PetSprite';
import type { PetAnimationState } from '../../core/pets';
import { useConfirmation } from '../ConfirmationProvider';

type CalendarEvent = {
  id: string;
  title: string;
  start: string;
  end: string;
  location: string;
  description: string;
  alertMinutes: number | null;
  recurrence: 'none' | 'daily' | 'weekly' | 'monthly';
};
type Draft = Omit<CalendarEvent, 'id'> & { id?: string; startLocal: string; endLocal: string };
type CalendarDefault = {
  mode: 'local' | 'ics' | 'google';
  path: string | null;
  googleAvailable: boolean;
};
const blank = (): Draft => ({
  title: '',
  start: '',
  end: '',
  startLocal: '',
  endLocal: '',
  location: '',
  description: '',
  alertMinutes: null,
  recurrence: 'none',
});
const localInput = (value: string) => {
  const date = new Date(value);
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(date.getTime() - offset).toISOString().slice(0, 16);
};
const range = (date: Date) => {
  const from = new Date(date.getFullYear(), date.getMonth(), 1);
  const to = new Date(date.getFullYear(), date.getMonth() + 1, 1);
  return { from: from.toISOString(), to: to.toISOString() };
};

export function CalendarScreen({
  selectedPet = 'character-female-a',
  petState = 'idle',
  reduceMotion = false,
  reduceMascot = false,
}: {
  selectedPet?: string;
  petState?: PetAnimationState;
  reduceMotion?: boolean;
  reduceMascot?: boolean;
}) {
  const requestConfirmation = useConfirmation();
  const [month, setMonth] = useState(() => new Date());
  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const [calendarDefault, setCalendarDefault] = useState<CalendarDefault>({
    mode: 'local',
    path: null,
    googleAvailable: false,
  });
  const reload = useCallback(async () => {
    if (!isTauri()) return;
    const bounds = range(month);
    setEvents(await invoke<CalendarEvent[]>('calendar_list_events', bounds));
  }, [month]);
  useEffect(() => {
    void reload().catch(() => setError('Não foi possível carregar a Agenda local.'));
  }, [reload]);
  useEffect(() => {
    if (isTauri())
      void invoke<CalendarDefault>('calendar_get_default')
        .then(setCalendarDefault)
        .catch(() => setError('Não foi possível carregar o calendário padrão.'));
  }, []);
  const dayEvents = useMemo(
    () => [...events].sort((a, b) => a.start.localeCompare(b.start)),
    [events],
  );
  function newEvent(day = new Date()) {
    const start = new Date(day);
    start.setHours(9, 0, 0, 0);
    const end = new Date(start);
    end.setHours(end.getHours() + 1);
    setDraft({
      ...blank(),
      startLocal: localInput(start.toISOString()),
      endLocal: localInput(end.toISOString()),
    });
  }
  function editEvent(event: CalendarEvent) {
    setDraft({
      ...event,
      id: event.id.split('@')[0],
      startLocal: localInput(event.start),
      endLocal: localInput(event.end),
    });
  }
  async function save(event: FormEvent) {
    event.preventDefault();
    if (!draft || busy) return;
    setBusy(true);
    setError('');
    try {
      const value: CalendarEvent = {
        id: draft.id ?? crypto.randomUUID(),
        title: draft.title.trim(),
        start: new Date(draft.startLocal).toISOString(),
        end: new Date(draft.endLocal).toISOString(),
        location: draft.location,
        description: draft.description,
        alertMinutes: draft.alertMinutes,
        recurrence: draft.recurrence,
      };
      await invoke('calendar_save_event', { event: value });
      setDraft(null);
      setMessage('Evento salvo na Agenda local do OdeKrai.');
      await reload();
    } catch {
      setError('Não foi possível salvar o evento. Confira início, fim e os campos obrigatórios.');
    } finally {
      setBusy(false);
    }
  }
  async function remove(event: CalendarEvent) {
    let accepted = false;
    try {
      accepted = await requestConfirmation({
        title: 'Apagar evento?',
        message: `Apagar “${event.title}” da Agenda local?`,
        destructive: true,
      });
    } catch {
      setError('Não foi possível mostrar a confirmação. O evento não foi apagado.');
      return;
    }
    if (!accepted) return;
    try {
      await invoke('calendar_delete_event', { id: event.id.split('@')[0] });
      setMessage('Evento apagado da Agenda local.');
      await reload();
    } catch {
      setError('Não foi possível apagar o evento.');
    }
  }
  async function importIcs() {
    const path = await open({
      multiple: false,
      filters: [{ name: 'Calendário iCalendar', extensions: ['ics'] }],
    });
    if (typeof path !== 'string') return;
    try {
      const count = await invoke<number>('calendar_import_ics', { path });
      setMessage(`${count} evento(s) importado(s) para a Agenda local.`);
      await reload();
    } catch {
      setError('Não foi possível importar o arquivo .ics.');
    }
  }
  async function exportIcs() {
    const path = await saveDialog({
      defaultPath: 'odekrai-agenda.ics',
      filters: [{ name: 'Calendário iCalendar', extensions: ['ics'] }],
    });
    if (!path) return;
    try {
      const count = await invoke<number>('calendar_export_ics', { path });
      setMessage(`${count} evento(s) exportado(s) para ${path}.`);
    } catch {
      setError('Não foi possível exportar o arquivo .ics. O destino precisa ser novo.');
    }
  }
  async function chooseCalendarFile() {
    const path = await saveDialog({
      defaultPath: 'odekrai-agenda-padrao.ics',
      filters: [{ name: 'Calendário iCalendar', extensions: ['ics'] }],
    });
    if (!path) return;
    try {
      await invoke('calendar_set_default', { mode: 'ics', path });
      setCalendarDefault({ ...calendarDefault, mode: 'ics', path });
      setMessage('O calendário padrão agora é o arquivo .ics escolhido.');
    } catch {
      setError('Não foi possível configurar o arquivo .ics padrão.');
    }
  }
  async function setLocalDefault() {
    try {
      await invoke('calendar_set_default', { mode: 'local', path: null });
      setCalendarDefault({ ...calendarDefault, mode: 'local', path: null });
      setMessage('O calendário padrão agora é a Agenda local do OdeKrai.');
    } catch {
      setError('Não foi possível salvar o calendário padrão.');
    }
  }
  async function setGoogleDefault() {
    try {
      await invoke('calendar_set_default', { mode: 'google', path: null });
      setCalendarDefault({ ...calendarDefault, mode: 'google' });
      setMessage(
        'O destino padrão das habilidades agora é o Google Calendar (calendário principal).',
      );
    } catch {
      setError(
        'Conecte o Google Calendar com o escopo calendar.events.owned antes de selecioná-lo.',
      );
    }
  }
  const weekdays = ['Dom', 'Seg', 'Ter', 'Qua', 'Qui', 'Sex', 'Sáb'];
  const first = new Date(month.getFullYear(), month.getMonth(), 1);
  const cells = Array.from(
    { length: 42 },
    (_, index) => new Date(first.getFullYear(), first.getMonth(), 1 - first.getDay() + index),
  );
  return (
    <div className="page">
      <PageHeader
        eyebrow="ORGANIZAÇÃO"
        title="Agenda"
        subtitle="Seus eventos ficam armazenados localmente no OdeKrai."
      />
      <section className="calendar-default">
        <strong>Calendário padrão</strong>
        <span>
          {calendarDefault.mode === 'local'
            ? 'Agenda do OdeKrai (local)'
            : calendarDefault.mode === 'google'
              ? 'Google Calendar (calendário principal)'
              : `Arquivo .ics: ${calendarDefault.path ?? 'não configurado'}`}
        </span>
        <button className="secondary-button" onClick={() => void setLocalDefault()}>
          Usar Agenda local
        </button>
        <button className="secondary-button" onClick={() => void chooseCalendarFile()}>
          Escolher arquivo .ics
        </button>
        <button
          className="secondary-button"
          onClick={() => void setGoogleDefault()}
          disabled={!calendarDefault.googleAvailable}
        >
          Usar Google Calendar
        </button>
        <small>
          {calendarDefault.googleAvailable
            ? 'Conexão autorizada para eventos Google.'
            : 'Conecte Google Calendar em Habilidades > Agenda e lembretes > Gerenciar para habilitar esta opção.'}
        </small>
      </section>
      <div className="calendar-toolbar">
        <button
          className="secondary-button"
          onClick={() => setMonth(new Date(month.getFullYear(), month.getMonth() - 1, 1))}
        >
          Mês anterior
        </button>
        <strong>{month.toLocaleDateString('pt-BR', { month: 'long', year: 'numeric' })}</strong>
        <button
          className="secondary-button"
          onClick={() => setMonth(new Date(month.getFullYear(), month.getMonth() + 1, 1))}
        >
          Próximo mês
        </button>
        <button className="secondary-button" onClick={() => void importIcs()}>
          Importar .ics
        </button>
        <button className="secondary-button" onClick={() => void exportIcs()}>
          Exportar .ics
        </button>
        <button className="primary-button" onClick={() => newEvent()}>
          Novo evento
        </button>
      </div>
      <div className="calendar-month" role="grid" aria-label="Visão mensal">
        {weekdays.map((day) => (
          <strong key={day}>{day}</strong>
        ))}
        {cells.map((date) => {
          const matches = events.filter((item) => {
            const start = new Date(item.start);
            return (
              start.getDate() === date.getDate() &&
              start.getMonth() === date.getMonth() &&
              start.getFullYear() === date.getFullYear()
            );
          });
          return (
            <button
              key={date.toISOString()}
              className={`calendar-day ${date.getMonth() !== month.getMonth() ? 'muted' : ''}`}
              onClick={() => {
                if (matches[0]) editEvent(matches[0]);
                else newEvent(date);
              }}
            >
              <span>{date.getDate()}</span>
              {matches.slice(0, 2).map((item) => (
                <small key={item.id}>
                  {new Date(item.start).toLocaleTimeString('pt-BR', {
                    hour: '2-digit',
                    minute: '2-digit',
                  })}{' '}
                  {item.title}
                </small>
              ))}
              {matches.length > 2 && <small>+{matches.length - 2}</small>}
            </button>
          );
        })}
      </div>
      <h2>Eventos do mês</h2>
      {dayEvents.length === 0 ? (
        <div className="calendar-empty-results">
          <PetSprite
            estado={petState}
            petId={selectedPet}
            reduceMotion={reduceMotion}
            staticImage={reduceMascot}
            className="empty-state-pet"
          />
          <p>Nenhum evento neste mês.</p>
        </div>
      ) : (
        <ul className="calendar-event-list">
          {dayEvents.map((event, index) => {
            const day = new Date(event.start).toLocaleDateString('pt-BR', {
              weekday: 'long',
              day: 'numeric',
              month: 'long',
            });
            const previous =
              index > 0
                ? new Date(dayEvents[index - 1].start).toLocaleDateString('pt-BR', {
                    weekday: 'long',
                    day: 'numeric',
                    month: 'long',
                  })
                : '';
            return (
              <Fragment key={event.id}>
                {day !== previous && (
                  <li className="calendar-day-heading">
                    <strong>{day}</strong>
                  </li>
                )}
                <li>
                  <div>
                    <strong>{event.title}</strong>
                    <span>
                      {new Date(event.start).toLocaleTimeString('pt-BR', {
                        hour: '2-digit',
                        minute: '2-digit',
                      })}{' '}
                      —{' '}
                      {new Date(event.end).toLocaleTimeString('pt-BR', {
                        hour: '2-digit',
                        minute: '2-digit',
                      })}
                      {event.location ? ` · ${event.location}` : ''}
                    </span>
                    {event.recurrence !== 'none' && <small>Recorrência: {event.recurrence}</small>}
                  </div>
                  <button className="secondary-button" onClick={() => editEvent(event)}>
                    Editar
                  </button>
                  <button className="secondary-button" onClick={() => void remove(event)}>
                    Apagar
                  </button>
                </li>
              </Fragment>
            );
          })}
        </ul>
      )}
      {message && <p role="status">{message}</p>}
      {error && <p role="alert">{error}</p>}
      {draft && (
        <form className="calendar-form" onSubmit={(e) => void save(e)}>
          <h2>{draft.id ? 'Editar evento' : 'Novo evento'}</h2>
          <label>
            Título
            <input
              required
              maxLength={200}
              value={draft.title}
              onChange={(e) => setDraft({ ...draft, title: e.target.value })}
            />
          </label>
          <div>
            <label>
              Início
              <input
                required
                type="datetime-local"
                value={draft.startLocal}
                onChange={(e) => setDraft({ ...draft, startLocal: e.target.value })}
              />
            </label>
            <label>
              Fim
              <input
                required
                type="datetime-local"
                value={draft.endLocal}
                onChange={(e) => setDraft({ ...draft, endLocal: e.target.value })}
              />
            </label>
          </div>
          <label>
            Local
            <input
              maxLength={500}
              value={draft.location}
              onChange={(e) => setDraft({ ...draft, location: e.target.value })}
            />
          </label>
          <label>
            Descrição
            <textarea
              maxLength={4000}
              value={draft.description}
              onChange={(e) => setDraft({ ...draft, description: e.target.value })}
            />
          </label>
          <div>
            <label>
              Alerta (minutos antes)
              <input
                type="number"
                min="0"
                max="10080"
                value={draft.alertMinutes ?? ''}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    alertMinutes: e.target.value === '' ? null : Number(e.target.value),
                  })
                }
              />
            </label>
            <label>
              Recorrência
              <select
                value={draft.recurrence}
                onChange={(e) =>
                  setDraft({ ...draft, recurrence: e.target.value as Draft['recurrence'] })
                }
              >
                <option value="none">Não repetir</option>
                <option value="daily">Diária</option>
                <option value="weekly">Semanal</option>
                <option value="monthly">Mensal</option>
              </select>
            </label>
          </div>
          <p>Alertas ficam pendentes até que haja um agendador disponível.</p>
          <button className="primary-button" disabled={busy}>
            Salvar evento
          </button>
          <button type="button" className="secondary-button" onClick={() => setDraft(null)}>
            Cancelar
          </button>
        </form>
      )}
    </div>
  );
}
