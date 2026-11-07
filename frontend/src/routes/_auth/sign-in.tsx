import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/_auth/sign-in')({
  component: SignIn,
})

function SignIn() {
  return <h1 className="text-2xl font-semibold text-text">Sign in</h1>
}
