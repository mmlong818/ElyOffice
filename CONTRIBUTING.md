# Contributing

## Opening a pull request on git.zacharyzhang.com

Forking is disabled on this server. Open pull requests with AGit instead:

1. Register an account at https://git.zacharyzhang.com
2. Clone, commit your changes, then push to a `refs/for/<target-branch>` ref:

   ```bash
   git clone https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice.git
   git push origin HEAD:refs/for/main -o topic=<short-name> -o title="<PR title>" -o description="<PR body>"
   ```

3. To update the pull request, push again with the same `topic`.
