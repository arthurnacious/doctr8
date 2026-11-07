import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/')({ component: Home })

function Home() {
  return (
    <main className="grid min-h-screen place-items-center bg-page p-8">
      <h1 className="text-2xl font-semibold text-text">doctr8</h1>
    </main>
  )
}
