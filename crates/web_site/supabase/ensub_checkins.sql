begin;

alter table public.ensub_checkins
  alter column word set not null,
  alter column rating set default 3,
  alter column rating set not null,
  alter column created_at set default timezone('utc'::text, now()),
  alter column created_at set not null;

alter table public.ensub_checkins
  add constraint ensub_checkins_word_length
    check (char_length(btrim(word)) between 1 and 120),
  add constraint ensub_checkins_context_length
    check (context_sentence is null or char_length(context_sentence) <= 1000),
  add constraint ensub_checkins_rating_range
    check (rating between 1 and 5),
  add constraint ensub_checkins_notes_length
    check (notes is null or char_length(notes) <= 1000);

create index ensub_checkins_created_at_desc_idx
  on public.ensub_checkins (created_at desc);

alter table public.ensub_checkins enable row level security;

drop policy if exists "Allow public read" on public.ensub_checkins;
drop policy if exists "Allow public insert" on public.ensub_checkins;

create policy "Shared check-ins are readable"
  on public.ensub_checkins
  for select
  to anon, authenticated
  using (true);

create policy "Shared check-ins are insertable"
  on public.ensub_checkins
  for insert
  to anon, authenticated
  with check (true);

revoke all privileges on table public.ensub_checkins from public, anon, authenticated;
grant select, insert on table public.ensub_checkins to anon, authenticated;

revoke all privileges on sequence public.ensub_checkins_id_seq from public, anon, authenticated;
grant usage on sequence public.ensub_checkins_id_seq to anon, authenticated;

commit;
