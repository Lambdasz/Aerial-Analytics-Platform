import { Card, H3, Tag } from "@blueprintjs/core";

export interface ModulePlaceholderProps {
  /** Module number as written in the project brief, e.g. 11. */
  readonly moduleNumber: number;
  /** Module name as written in the project brief. */
  readonly title: string;
}

/**
 * Empty page used as the starting point for every module.
 *
 * The module owner replaces the body of their own page file; this component is
 * only the shared scaffold so every module starts from the same shape.
 */
export function ModulePlaceholder({ moduleNumber, title }: ModulePlaceholderProps) {
  return (
    <section className="module-page">
      <div className="module-page__header">
        <Tag minimal>Modul {moduleNumber}</Tag>
        <H3 className="module-page__title">{title}</H3>
      </div>

      <Card className="module-page__body">
        <p>
          Halaman ini masih kosong. Isi halaman dikerjakan oleh pemilik modul di dalam folder{" "}
          <code>src/modules/</code> miliknya.
        </p>
      </Card>
    </section>
  );
}
